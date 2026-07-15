use std::{env, error::Error, fs, io, path::PathBuf};

use schemars::schema::{RootSchema, Schema};
use typify::{TypeSpace, TypeSpaceSettings};

const REFERENCE_REF: &str = "luma-forge://schema/reference";
const REFERENCE_DEFS_REF: &str = "#/$defs/Reference";

type CodegenResult<T> = Result<T, Box<dyn Error>>;

pub fn generate() -> CodegenResult<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "manifest dir should have repo root parent",
        )
    })?;
    let schema_dir = repo_root.join("bundled/catalog/schemas");
    println!("cargo:rerun-if-changed={}", schema_dir.display());

    let mut schemas = Vec::new();
    for entry in fs::read_dir(&schema_dir)? {
        let path = entry?.path();
        if path.is_file() {
            schemas.push(path);
        }
    }
    schemas.sort();

    let mut settings = TypeSpaceSettings::default();
    settings
        .with_conversion(
            schemars::schema::SchemaObject::default(),
            "serde_json::Value",
            [typify::TypeSpaceImpl::Display].into_iter(),
        )
        .with_struct_builder(false);
    let mut definitions = Vec::new();

    for schema_path in schemas {
        println!("cargo:rerun-if-changed={}", schema_path.display());
        let schema_text = fs::read_to_string(&schema_path)?;
        let root_schema: RootSchema = serde_json::from_str(&schema_text)?;
        let mut schema_value = serde_json::to_value(root_schema.schema)?;
        rewrite_reference_refs(&mut schema_value);
        let schema: Schema = serde_json::from_value(schema_value)?;
        let title = schema_title(&schema)?;
        definitions.push((title, schema));
    }

    let mut type_space = TypeSpace::new(&settings);
    type_space.add_ref_types(definitions)?;

    let generated = type_space.to_stream().to_string();
    let syntax = syn::parse_file(&generated)?;
    let formatted = prettyplease::unparse(&syntax);
    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("bundled_generated.rs");
    fs::write(out_path, formatted)?;
    Ok(())
}

fn schema_title(schema: &Schema) -> CodegenResult<String> {
    match schema {
        Schema::Object(schema) => schema
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.title.clone())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "schema should have title").into()
            }),
        Schema::Bool(_) => {
            Err(io::Error::new(io::ErrorKind::InvalidData, "schema should have title").into())
        }
    }
}

fn rewrite_reference_refs(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                rewrite_reference_refs(item);
            }
        }
        serde_json::Value::Object(map) => {
            if map.get("$ref").and_then(serde_json::Value::as_str) == Some(REFERENCE_REF) {
                map.insert(
                    "$ref".to_string(),
                    serde_json::Value::String(REFERENCE_DEFS_REF.to_string()),
                );
            }

            for value in map.values_mut() {
                rewrite_reference_refs(value);
            }
        }
        _ => {}
    }
}
