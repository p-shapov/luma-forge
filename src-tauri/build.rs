use std::{
    env, fs,
    path::{Path, PathBuf},
};

use typify::{TypeSpace, TypeSpaceSettings};

#[path = "src/infra/bundled/validation.rs"]
mod validation;
#[path = "src/infra/bundled/validation_errors.rs"]
mod validation_errors;

fn main() {
    println!("cargo::rerun-if-changed=../bundled");
    println!("cargo::rerun-if-changed=schemas/bundled");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let schemas_dir = manifest_dir.join("schemas/bundled");
    let bundled_dir = manifest_dir.join("../bundled");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));

    let schemas = load_schema_files(&schemas_dir);
    let layouts_dir = schemas_dir.join("layouts");
    let layout_schema = schemas
        .iter()
        .find(|schema| schema.id == "luma-forge://schemas/bundled/layout.schema.json")
        .unwrap_or_else(|| panic!("schemas/bundled/layout.schema.json: schema missing"));
    let layout_validator = jsonschema::validator_for(&layout_schema.json)
        .unwrap_or_else(|error| panic!("layout schema validator failed: {error}"));
    let layouts = load_layout_files(&layouts_dir, &layout_validator);
    let entity_schema_ids = entity_schema_ids(&layouts);
    validate_entity_schema_ids(&schemas, &entity_schema_ids);
    generate_types(&schemas, &entity_schema_ids, &out_dir);
    let assets = validation::validate_bundled_catalog(&bundled_dir, &schemas, &layouts)
        .unwrap_or_else(|error| panic!("bundled catalog validation failed: {error}"));
    validation::validate_cross_file_assets(&assets)
        .unwrap_or_else(|error| panic!("bundled catalog validation failed: {error}"));
    generate_manifest(&assets, &out_dir);

    tauri_build::build();
}

fn load_schema_files(schemas_dir: &Path) -> Vec<validation::SchemaDocument> {
    let mut schemas = Vec::new();
    let entries = fs::read_dir(schemas_dir).unwrap_or_else(|error| {
        panic!(
            "{}: directory traversal failed: {error}",
            schemas_dir.display()
        )
    });
    let mut paths = Vec::new();

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| {
                panic!("{}: directory entry failed: {error}", schemas_dir.display())
            })
            .path();
        paths.push(path);
    }

    paths.sort();

    for path in paths {
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("layouts") {
                continue;
            }
            panic!("{}: unexpected schema subdirectory", path.display());
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".schema.json"))
        {
            panic!("{}: unexpected schema JSON file", path.display());
        }

        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: schema read failed: {error}", path.display()));
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{}: schema parse failed: {error}", path.display()));
        let id = json
            .get("$id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{}: schema missing $id", path.display()))
            .to_string();
        schemas.push(validation::SchemaDocument { id, json });
    }
    schemas
}

fn entity_schema_ids(layouts: &[validation::LayoutSpec]) -> std::collections::BTreeSet<String> {
    layouts
        .iter()
        .flat_map(|layout| layout.files.values().map(|file| file.schema.clone()))
        .collect()
}

fn validate_entity_schema_ids(
    schemas: &[validation::SchemaDocument],
    entity_schema_ids: &std::collections::BTreeSet<String>,
) {
    let schema_ids: std::collections::BTreeSet<&str> =
        schemas.iter().map(|schema| schema.id.as_str()).collect();
    if let Some(missing_schema_id) = entity_schema_ids
        .iter()
        .find(|schema_id| !schema_ids.contains(schema_id.as_str()))
    {
        panic!("layout references missing schema: {missing_schema_id}");
    }
}

fn load_layout_files(
    layouts_dir: &Path,
    layout_validator: &jsonschema::Validator,
) -> Vec<validation::LayoutSpec> {
    let mut layouts = Vec::new();
    let entries = fs::read_dir(layouts_dir).unwrap_or_else(|error| {
        panic!(
            "{}: directory traversal failed: {error}",
            layouts_dir.display()
        )
    });
    let mut paths = Vec::new();

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| {
                panic!("{}: directory entry failed: {error}", layouts_dir.display())
            })
            .path();
        paths.push(path);
    }

    paths.sort();

    for path in paths {
        if path.is_dir() {
            panic!("{}: unexpected layout subdirectory", path.display());
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: layout read failed: {error}", path.display()));
        let json: serde_json::Value = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{}: layout parse failed: {error}", path.display()));
        if let Err(error) = layout_validator.validate(&json) {
            panic!(
                "{}: layout schema validation failed: {error}",
                path.display()
            );
        }
        let relative = path
            .strip_prefix(layouts_dir.parent().unwrap_or(layouts_dir))
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        layouts.push(
            validation::LayoutSpec::from_json(&relative, json)
                .unwrap_or_else(|error| panic!("bundled layout validation failed: {error}")),
        );
    }

    layouts
}

fn generate_types(
    schemas: &[validation::SchemaDocument],
    entity_schema_ids: &std::collections::BTreeSet<String>,
    out_dir: &Path,
) {
    let mut type_space = TypeSpace::new(&TypeSpaceSettings::default());
    let reference_schema = schemas
        .iter()
        .find(|schema| schema.id == "luma-forge://schemas/bundled/reference.schema.json")
        .unwrap_or_else(|| panic!("reference schema missing"));
    let reference_root: schemars::schema::RootSchema =
        serde_json::from_value(reference_schema.json.clone()).unwrap_or_else(|error| {
            panic!("{}: schema decode failed: {error}", reference_schema.id)
        });
    type_space
        .add_ref_types([(
            "Reference",
            schemars::schema::Schema::Object(reference_root.schema.clone()),
        )])
        .expect("typify failed to add bundled reference schema");
    for schema in schemas
        .iter()
        .filter(|schema| entity_schema_ids.contains(&schema.id))
    {
        let root_schema: schemars::schema::RootSchema =
            serde_json::from_value(typify_schema_json(schema.json.clone()))
                .unwrap_or_else(|error| panic!("{}: schema decode failed: {error}", schema.id));
        type_space
            .add_root_schema(root_schema)
            .expect("typify failed to add bundled schema");
    }
    let syntax = syn::parse2(type_space.to_stream()).expect("typify generated invalid tokens");
    let contents = prettyplease::unparse(&syntax);
    fs::write(out_dir.join("bundled_types.rs"), contents)
        .expect("failed to write bundled_types.rs");
}

fn generate_manifest(files: &[validation::BundledJsonFile], out_dir: &Path) {
    let mut contents = String::from("pub const BUNDLED_ASSETS: &[(&str, &str)] = &[\n");
    for file in files {
        contents.push_str("    (");
        contents.push_str(&format!("{:?}", file.path));
        contents.push_str(", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../bundled/");
        contents.push_str(&file.path);
        contents.push_str("\"))),\n");
    }
    contents.push_str("];\n");
    fs::write(out_dir.join("bundled_manifest.rs"), contents)
        .expect("failed to write bundled_manifest.rs");
}

fn typify_schema_json(mut json: serde_json::Value) -> serde_json::Value {
    rewrite_reference_schema_refs(&mut json);
    json
}

fn rewrite_reference_schema_refs(json: &mut serde_json::Value) {
    match json {
        serde_json::Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if key == "$ref"
                    && value.as_str() == Some("luma-forge://schemas/bundled/reference.schema.json")
                {
                    *value = serde_json::Value::String("#/definitions/Reference".to_string());
                    continue;
                }
                rewrite_reference_schema_refs(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                rewrite_reference_schema_refs(value);
            }
        }
        _ => {}
    }
}
