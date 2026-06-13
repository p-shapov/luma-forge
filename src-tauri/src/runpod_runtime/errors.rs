use serde::{Deserialize, Serialize};

use crate::{
    domain::runpod::{RunpodLifecycleError},
    secrets_storage::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodRuntimeError {
    #[error("lifecycle error")]
    LifecycleError(#[from] RunpodLifecycleError),
    #[error("runpod api key unavailable")]
    RunpodApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("hugging face api key unavailable")]
    HuggingFaceApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow catalog invalid")]
    WorkflowCatalogInvalid(#[from] WorkflowCatalogError),
    #[error("workspace catalog invalid")]
    WorkspaceCatalogInvalid(#[from] WorkspaceCatalogError),
    #[error("invalid runtime state: {message}")]
    InvalidRuntimeState { message: String },
}

pub fn invalid_runtime_state<E: std::error::Error>(error: E) -> RunpodRuntimeError {
    RunpodRuntimeError::InvalidRuntimeState { message: error.to_string() }
}

pub fn runpod_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::RunpodApiKeyUnavailable(error)
}

pub fn hugging_face_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::HuggingFaceApiKeyUnavailable(error)
}
