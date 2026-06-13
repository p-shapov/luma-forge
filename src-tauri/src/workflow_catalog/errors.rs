use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowCatalogError {
    #[error("workflow catalog parse failed: {message}")]
    ParseFailed { message: String },
    #[error("workflow catalog validation failed: {message}")]
    ValidationFailed { message: String },
}

pub fn parse_failed<E: std::error::Error>(error: E) -> WorkflowCatalogError {
    WorkflowCatalogError::ParseFailed {
        message: error.to_string(),
    }
}

pub fn validation_failed<E: std::error::Error>(error: E) -> WorkflowCatalogError {
    WorkflowCatalogError::ValidationFailed {
        message: error.to_string(),
    }
}
