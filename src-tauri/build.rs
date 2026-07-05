use std::{env, fs, path::PathBuf};

use schemars::schema::{RootSchema, Schema};
use typify::{TypeSpace, TypeSpaceSettings};

const REFERENCE_REF: &str = "luma-forge://schema/reference";
const REFERENCE_DEFS_REF: &str = "#/$defs/Reference";

fn main() {
    generate_bundled_types();
    tauri_build::build()
}

fn generate_bundled_types() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let schema_dir = manifest_dir
        .parent()
        .expect("repo root")
        .join("new_bundled/catalog/schemas");
    println!("cargo:rerun-if-changed={}", schema_dir.display());

    let mut schemas = fs::read_dir(&schema_dir)
        .expect("bundled schema dir should be readable")
        .map(|entry| {
            entry
                .expect("bundled schema entry should be readable")
                .path()
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
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
        let schema_text = fs::read_to_string(&schema_path).expect("schema should be readable");
        let root_schema: RootSchema =
            serde_json::from_str(&schema_text).expect("schema should parse");
        let mut schema_value =
            serde_json::to_value(root_schema.schema).expect("schema should serialize");
        rewrite_reference_refs(&mut schema_value);
        let schema: Schema =
            serde_json::from_value(schema_value).expect("schema should deserialize");
        let title = match &schema {
            Schema::Object(schema) => schema
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.title.clone())
                .expect("schema should have title"),
            Schema::Bool(_) => panic!("schema should have title"),
        };
        definitions.push((title, schema));
    }

    let mut type_space = TypeSpace::new(&settings);
    type_space
        .add_ref_types(definitions)
        .expect("schemas should generate Rust types");

    let generated = type_space.to_stream().to_string();
    let syntax = syn::parse_file(&generated).expect("generated Rust should parse");
    let formatted = prettyplease::unparse(&syntax);
    let out_path =
        PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("bundled_generated.rs");
    fs::write(out_path, formatted).expect("generated bundled DTOs should write");
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
