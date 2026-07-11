use std::{env, error::Error, fs, io, path::PathBuf};

use schemars::schema::{RootSchema, SchemaObject};
use typify::{TypeSpace, TypeSpaceSettings};

type CodegenResult<T> = Result<T, Box<dyn Error>>;

pub fn generate() -> CodegenResult<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let schema_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "repo root should exist"))?
        .join("openapi");

    generate_schema(&schema_dir.join("runpod.json"), "runpod_generated.rs")?;
    generate_schema(
        &schema_dir.join("hugging-face.json"),
        "hugging_face_generated.rs",
    )
}

fn generate_schema(schema_path: &std::path::Path, output_name: &str) -> CodegenResult<()> {
    println!("cargo:rerun-if-changed={}", schema_path.display());

    let root_schema: RootSchema = serde_json::from_str(&fs::read_to_string(schema_path)?)?;
    let mut settings = TypeSpaceSettings::default();
    settings
        .with_conversion(
            SchemaObject::default(),
            "serde_json::Value",
            [typify::TypeSpaceImpl::Display].into_iter(),
        )
        .with_struct_builder(false);
    let mut type_space = TypeSpace::new(&settings);
    type_space.add_ref_types(root_schema.definitions)?;

    let syntax = syn::parse_file(&type_space.to_stream().to_string())?;
    let output_path = PathBuf::from(env::var("OUT_DIR")?).join(output_name);
    fs::write(output_path, prettyplease::unparse(&syntax))?;
    Ok(())
}
