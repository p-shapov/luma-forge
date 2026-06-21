use serde::{Deserialize, Serialize};

use crate::{
    lifecycle_journal::LifecycleJournalError, provider::errors::ProviderApiError,
    runtime_catalog::RuntimeCatalogError, secrets::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError, workspace_catalog::WorkspaceCatalogError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceError {
    #[error("provider api error: {0}")]
    ProviderApiError(#[from] ProviderApiError),
    #[error("runtime provider api key unavailable: {0}")]
    RuntimeProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow provider api key unavailable: {0}")]
    WorkflowProviderApiKeyUnavailable(#[source] SecretsStorageError),
    #[error("workflow catalog invalid: {0}")]
    WorkflowCatalogInvalid(#[from] WorkflowCatalogError),
    #[error("runtime catalog invalid: {0}")]
    RuntimeCatalogInvalid(#[from] RuntimeCatalogError),
    #[error("workspace catalog invalid: {0}")]
    WorkspaceCatalogInvalid(#[from] WorkspaceCatalogError),
    #[error("lifecycle journal invalid: {0}")]
    LifecycleJournalInvalid(#[from] LifecycleJournalError),
    #[error("workspace was not found: {workspace_id}")]
    WorkspaceNotFound { workspace_id: String },
    #[error("workspace already has a running lifecycle operation: {operation_id}")]
    LifecycleOperationAlreadyRunning { operation_id: String },
    #[error("invalid runtime state: {message}")]
    InvalidState { message: String },
}

pub fn invalid_state(message: impl Into<String>) -> WorkspaceError {
    WorkspaceError::InvalidState {
        message: message.into(),
    }
}

pub fn workspace_not_found(workspace_id: impl Into<String>) -> WorkspaceError {
    WorkspaceError::WorkspaceNotFound {
        workspace_id: workspace_id.into(),
    }
}
