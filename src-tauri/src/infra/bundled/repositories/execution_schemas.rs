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

fn parse_execution_schema(
    path: &str,
    text: &str,
) -> Result<BundledExecutionSchema, BundledCatalogError> {
    let schema: generated::ExecutionSchema = serde_json::from_str(text)
        .map_err(|error| BundledCatalogError::corrupt_asset(path, error.to_string()))?;
    Ok(BundledExecutionSchema {
        id: schema.id.into(),
        revision: schema.revision.into(),
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
