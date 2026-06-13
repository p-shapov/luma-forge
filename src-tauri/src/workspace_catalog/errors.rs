use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCatalogError {
    #[error("workspace catalog storage unavailable: {message}")]
    StorageUnavailable { message: String },
    #[error("workspace catalog schema is invalid: {message}")]
    SchemaInvalid { message: String },
    #[error("workspace catalog data is invalid: {message}")]
    DataInvalid { message: String },
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("workspace was not found")]
    WorkspaceNotFound,
}

pub fn storage_unavailable_message(message: impl Into<String>) -> WorkspaceCatalogError {
    WorkspaceCatalogError::StorageUnavailable {
        message: message.into(),
    }
}

pub fn schema_invalid_message(message: impl Into<String>) -> WorkspaceCatalogError {
    WorkspaceCatalogError::SchemaInvalid {
        message: message.into(),
    }
}

pub fn data_invalid_message(message: impl Into<String>) -> WorkspaceCatalogError {
    WorkspaceCatalogError::DataInvalid {
        message: message.into(),
    }
}

pub fn storage_unavailable_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    storage_unavailable_message(error.to_string())
}

pub fn schema_invalid_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    schema_invalid_message(error.to_string())
}

pub fn data_invalid_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    data_invalid_message(error.to_string())
}
