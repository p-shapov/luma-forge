use serde::{Deserialize, Serialize};

use crate::{
    runtime_catalog::RuntimeCatalogError, secrets_storage::SecretsStorageError, shared::ApiError,
    workflow_catalog::WorkflowCatalogError, workspace_catalog::WorkspaceCatalogError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum RunpodRuntimeError {
    #[error("runpod api error")]
    RunpodApiError(#[from] ApiError),
    #[error("runpod api key unavailable: {0}")]
    RunpodApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("hugging face api key unavailable: {0}")]
    HuggingFaceApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow catalog invalid")]
    WorkflowCatalogInvalid(#[from] WorkflowCatalogError),
    #[error("runtime catalog invalid")]
    RuntimeCatalogInvalid(#[from] RuntimeCatalogError),
    #[error("workspace catalog invalid")]
    WorkspaceCatalogInvalid(#[from] WorkspaceCatalogError),
    #[error("provisioner worker unavailable: {message}")]
    ProvisionerWorkerUnavailable { message: String },
    #[error("provisioner worker response invalid: {message}")]
    ProvisionerWorkerResponseInvalid { message: String },
    #[error("provisioner worker failed: {message}")]
    ProvisionerWorkerFailed { message: String },
    #[error("workspace was not found: {workspace_id}")]
    WorkspaceNotFound { workspace_id: String },
    #[error("workspace already has a running lifecycle operation: {workspace_id}")]
    LifecycleOperationAlreadyRunning { workspace_id: String },
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

pub fn workspace_not_found(workspace_id: impl Into<String>) -> RunpodRuntimeError {
    RunpodRuntimeError::WorkspaceNotFound {
        workspace_id: workspace_id.into(),
    }
}

pub fn lifecycle_operation_already_running(workspace_id: impl Into<String>) -> RunpodRuntimeError {
    RunpodRuntimeError::LifecycleOperationAlreadyRunning {
        workspace_id: workspace_id.into(),
    }
}

pub fn runpod_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::RunpodApiKeyUnavailable(error)
}

pub fn hugging_face_api_key_unavailable(error: SecretsStorageError) -> RunpodRuntimeError {
    RunpodRuntimeError::HuggingFaceApiKeyUnavailable(error)
}
