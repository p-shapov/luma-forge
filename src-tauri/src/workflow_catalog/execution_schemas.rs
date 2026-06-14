use std::collections::HashSet;

use serde::Deserialize;

use super::errors::WorkflowCatalogError;

const EXECUTION_SCHEMAS_JSON: &str = include_str!("../../../bundled/execution-schemas.json");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionSchemaRegistry {
    pub execution_schemas: Vec<ExecutionSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionSchema {
    pub id: String,
    pub revisions: Vec<ExecutionSchemaRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionSchemaRevision {
    pub version: String,
    pub inputs: Vec<ExecutionSchemaInput>,
    pub outputs: ExecutionSchemaOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionSchemaInput {
    pub id: String,
    #[serde(rename = "type")]
    pub input_type: String,
    pub required: bool,
    pub max_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionSchemaOutputs {
    #[serde(rename = "type")]
    pub output_type: String,
}

pub fn read_bundled_execution_schema_registry(
) -> Result<ExecutionSchemaRegistry, WorkflowCatalogError> {
    serde_json::from_str(EXECUTION_SCHEMAS_JSON).map_err(|error| {
        WorkflowCatalogError::ParseFailed {
            message: error.to_string(),
        }
    })
}

impl ExecutionSchemaRegistry {
    pub fn find_revision(&self, id: &str, version: &str) -> Option<&ExecutionSchemaRevision> {
        self.execution_schemas
            .iter()
            .find(|schema| schema.id == id)
            .and_then(|schema| {
                schema
                    .revisions
                    .iter()
                    .find(|revision| revision.version == version)
            })
    }
}

pub fn validate_execution_schema_registry(
    registry: &ExecutionSchemaRegistry,
) -> Result<(), WorkflowCatalogError> {
    let mut schema_ids = HashSet::new();
    for schema in &registry.execution_schemas {
        if schema.id.trim().is_empty()
            || !schema_ids.insert(schema.id.as_str())
            || schema.revisions.is_empty()
        {
            return validation_error("execution schema id is empty, duplicate, or revisions are empty");
        }
        let mut revision_versions = HashSet::new();
        for revision in &schema.revisions {
            validate_schema_revision(revision, &mut revision_versions)?;
        }
    }
    Ok(())
}

fn validate_schema_revision<'a>(
    revision: &'a ExecutionSchemaRevision,
    revision_versions: &mut HashSet<&'a str>,
) -> Result<(), WorkflowCatalogError> {
    if revision.version.trim().is_empty() || !revision_versions.insert(revision.version.as_str()) {
        return validation_error("execution schema revision version is empty or duplicate");
    }
    if revision.inputs.is_empty() || revision.outputs.output_type.trim().is_empty() {
        return validation_error("execution schema inputs or outputs are empty");
    }
    let mut input_ids = HashSet::new();
    for input in &revision.inputs {
        if input.id.trim().is_empty()
            || !input_ids.insert(input.id.as_str())
            || is_secret_like(&input.id)
            || input.input_type != "string"
            || input.max_length == Some(0)
        {
            return validation_error("execution schema input is invalid");
        }
    }
    Ok(())
}

pub fn required_input_ids(revision: &ExecutionSchemaRevision) -> HashSet<&str> {
    revision
        .inputs
        .iter()
        .filter(|input| input.required)
        .map(|input| input.id.as_str())
        .collect()
}

pub fn input_ids(revision: &ExecutionSchemaRevision) -> HashSet<&str> {
    revision.inputs.iter().map(|input| input.id.as_str()).collect()
}

fn is_secret_like(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("secret")
        || value.contains("token")
        || value.contains("password")
        || value.contains("api_key")
        || value.contains("apikey")
        || value.contains("credential")
}

fn validation_error<T>(message: &'static str) -> Result<T, WorkflowCatalogError> {
    Err(WorkflowCatalogError::ValidationFailed {
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_registry() -> ExecutionSchemaRegistry {
        ExecutionSchemaRegistry {
            execution_schemas: vec![ExecutionSchema {
                id: "text-to-image".to_string(),
                revisions: vec![ExecutionSchemaRevision {
                    version: "1.0.0".to_string(),
                    inputs: vec![ExecutionSchemaInput {
                        id: "prompt".to_string(),
                        input_type: "string".to_string(),
                        required: true,
                        max_length: Some(4000),
                    }],
                    outputs: ExecutionSchemaOutputs {
                        output_type: "image_set".to_string(),
                    },
                }],
            }],
        }
    }

    #[test]
    fn bundled_execution_schema_registry_is_valid() {
        let registry = read_bundled_execution_schema_registry().expect("registry should parse");

        assert_eq!(validate_execution_schema_registry(&registry), Ok(()));
        assert!(registry.find_revision("text-to-image", "1.0.0").is_some());
    }

    #[test]
    fn validation_rejects_secret_like_input_ids() {
        let mut registry = valid_registry();
        registry.execution_schemas[0].revisions[0].inputs[0].id = "api_key".to_string();

        assert_eq!(
            validate_execution_schema_registry(&registry),
            Err(WorkflowCatalogError::ValidationFailed {
                message: "execution schema input is invalid".to_string(),
            })
        );
    }
}
