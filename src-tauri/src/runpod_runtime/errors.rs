use serde::{Deserialize, Serialize};

use crate::{
    secrets_storage::SecretsStorageError, shared::ApiError, workflow_catalog::WorkflowCatalogError,
    workspace_catalog::WorkspaceCatalogError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodRuntimeError {
    #[error("runpod api error")]
    RunpodApiError(#[from] ApiError),
    #[error("runpod api key unavailable")]
    RunpodApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("hugging face api key unavailable")]
    HuggingFaceApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow catalog invalid")]
    WorkflowCatalogInvalid(#[from] WorkflowCatalogError),
    #[error("workspace catalog invalid")]
    WorkspaceCatalogInvalid(#[from] WorkspaceCatalogError),
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable { message: String },
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid { message: String },
    #[error("provisioner worker failed")]
    ProvisionerWorkerFailed { message: String },
    #[error("invalid runtime state: {message}")]
    InvalidRuntimeState { message: String },
}

pub fn invalid_runtime_state_message(message: impl Into<String>) -> RunpodRuntimeError {
    RunpodRuntimeError::InvalidRuntimeState {
        message: message.into(),
    }
}

pub fn invalid_runtime_state_error<E: std::error::Error>(error: E) -> RunpodRuntimeError {
    invalid_runtime_state_message(error.to_string())
}

pub fn runpod_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::RunpodApiKeyUnavailable(error)
}

pub fn hugging_face_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::HuggingFaceApiKeyUnavailable(error)
}
