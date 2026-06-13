use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleJournalError {
    #[error("operation not found")]
    OperationNotFound,
    #[error("running operation exists")]
    RunningOperationExists,
    #[error("storage unavailable: {message}")]
    StorageUnavailable { message: String },
    #[error("schema invalid: {message}")]
    SchemaInvalid { message: String },
    #[error("data invalid: {message}")]
    DataInvalid { message: String },
}

pub fn storage_unavailable_message(message: impl Into<String>) -> LifecycleJournalError {
    LifecycleJournalError::StorageUnavailable {
        message: message.into(),
    }
}

pub fn schema_invalid_message(message: impl Into<String>) -> LifecycleJournalError {
    LifecycleJournalError::SchemaInvalid {
        message: message.into(),
    }
}

pub fn data_invalid_message(message: impl Into<String>) -> LifecycleJournalError {
    LifecycleJournalError::DataInvalid {
        message: message.into(),
    }
}

pub fn storage_unavailable_error(error: impl std::fmt::Display) -> LifecycleJournalError {
    storage_unavailable_message(error.to_string())
}

pub fn schema_invalid_error(error: impl std::fmt::Display) -> LifecycleJournalError {
    schema_invalid_message(error.to_string())
}

pub fn data_invalid_error(error: impl std::fmt::Display) -> LifecycleJournalError {
    data_invalid_message(error.to_string())
}
