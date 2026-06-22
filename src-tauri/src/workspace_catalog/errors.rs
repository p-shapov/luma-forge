use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    thiserror::Error,
    luma_diagnostic::DiagnosticCode,
)]
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

pub fn data_invalid_message(message: impl Into<String>) -> WorkspaceCatalogError {
    WorkspaceCatalogError::DataInvalid {
        message: message.into(),
    }
}

pub fn storage_unavailable_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    WorkspaceCatalogError::StorageUnavailable {
        message: error.to_string(),
    }
}

pub fn schema_invalid_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    WorkspaceCatalogError::SchemaInvalid {
        message: error.to_string(),
    }
}

pub fn data_invalid_error(error: impl std::fmt::Display) -> WorkspaceCatalogError {
    WorkspaceCatalogError::DataInvalid {
        message: error.to_string(),
    }
}
