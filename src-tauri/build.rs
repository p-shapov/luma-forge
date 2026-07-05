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
    generate_types(&schemas, &out_dir);
    let assets = validation::validate_bundled_catalog(&bundled_dir, &schemas)
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

fn generate_types(schemas: &[validation::SchemaDocument], out_dir: &Path) {
    let mut type_space = TypeSpace::new(&TypeSpaceSettings::default());
    for schema in schemas {
        let root_schema: schemars::schema::RootSchema = serde_json::from_value(schema.json.clone())
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

fn generate_manifest(assets: &[validation::BundledAsset], out_dir: &Path) {
    let mut contents = String::from("pub const BUNDLED_ASSETS: &[(&str, &str)] = &[\n");
    for asset in assets {
        contents.push_str("    (");
        contents.push_str(&format!("{:?}", asset.path));
        contents.push_str(", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../bundled/");
        contents.push_str(&asset.path);
        contents.push_str("\"))),\n");
    }
    contents.push_str("];\n");
    fs::write(out_dir.join("bundled_manifest.rs"), contents)
        .expect("failed to write bundled_manifest.rs");
}
