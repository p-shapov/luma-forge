use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCatalogError {
    #[error("runtime catalog parse failed: {message}")]
    ParseFailed { message: String },
    #[error("runtime catalog validation failed: {message}")]
    ValidationFailed { message: String },
}
