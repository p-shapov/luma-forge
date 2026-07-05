use crate::infra::bundled::{
    errors::BundledCatalogError,
    generated,
    models::{BundledExecutionInput, BundledExecutionSchema},
};

#[derive(Debug, Clone, Default)]
pub struct BundledExecutionSchemaRepository;

impl BundledExecutionSchemaRepository {
    pub fn new() -> Self {
        Self
    }

    pub fn list(&self) -> Result<Vec<BundledExecutionSchema>, BundledCatalogError> {
        generated::BUNDLED_ASSETS
            .iter()
            .filter(|(path, _)| path.starts_with("execution_schemas/"))
            .map(|(path, text)| parse_execution_schema(path, text))
            .collect()
    }

    pub fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<BundledExecutionSchema>, BundledCatalogError> {
        let path = format!("execution_schemas/{id}/{revision}.json");
        generated::BUNDLED_ASSETS
            .iter()
            .find_map(|(asset_path, text)| (*asset_path == path).then_some(*text))
            .map(|text| parse_execution_schema(&path, text))
            .transpose()
    }
}

fn identity_from_revision_path(
    path: &str,
    prefix: &str,
) -> Result<(String, String), BundledCatalogError> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [actual_prefix, id, file] if *actual_prefix == prefix => {
            let Some(revision) = file.strip_suffix(".json") else {
                return Err(BundledCatalogError::corrupt_asset(
                    path,
                    "revision file is invalid",
                ));
            };
            Ok(((*id).to_string(), revision.to_string()))
        }
        _ => Err(BundledCatalogError::corrupt_asset(
            path,
            "bundled path is invalid",
        )),
    }
}

fn parse_execution_schema(
    path: &str,
    text: &str,
) -> Result<BundledExecutionSchema, BundledCatalogError> {
    let schema: generated::ExecutionSchema = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    let (id, revision) = identity_from_revision_path(path, "execution_schemas")?;
    Ok(BundledExecutionSchema {
        id,
        revision,
        inputs: schema
            .inputs
            .into_iter()
            .map(|input| BundledExecutionInput {
                id: input.id.into(),
                input_type: "string".to_string(),
                required: input.required,
                max_length: input.max_length.map(|value| value.get()),
            })
            .collect(),
        output_type: schema.outputs.type_.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_execution_schema_uses_identity_from_path() {
        let schema = parse_execution_schema(
            "execution_schemas/example/1.2.3.json",
            r#"{
              "$schema":"luma-forge://schemas/bundled/execution_schema.schema.json",
              "inputs":[{"id":"prompt","type":"string","required":true,"max_length":4000}],
              "outputs":{"type":"image_set"}
            }"#,
        )
        .expect("schema should parse");

        assert_eq!(schema.id, "example");
        assert_eq!(schema.revision, "1.2.3");
    }

    #[test]
    fn get_returns_none_for_missing_execution_schema() {
        let repository = BundledExecutionSchemaRepository::new();

        assert_eq!(
            repository
                .get("missing-execution-schema", "9.9.9")
                .expect("lookup should succeed"),
            None
        );
    }
}
