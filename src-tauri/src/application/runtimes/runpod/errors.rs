use crate::application::{
    lifecycle::{ports::LifecycleOperationRepositoryError, LifecycleError},
    secrets::SecretStoreError,
};

use super::ports::{
    RunpodRuntimeCatalogError, RunpodRuntimeProviderError, RunpodRuntimeRepositoryError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeError {
    #[error("workspace was not found")]
    WorkspaceNotFound,
    #[error("workflow was not found")]
    WorkflowNotFound,
    #[error("runtime is already provisioned")]
    AlreadyProvisioned,
    #[error("runtime is failed and must be cleaned up")]
    RuntimeFailed,
    #[error("runtime operation is already in progress")]
    OperationInProgress,
    #[error("runtime is not provisioned")]
    NotProvisioned,
    #[error("required credential is not configured")]
    CredentialMissing,
    #[error("runtime provider is unavailable")]
    ProviderUnavailable,
    #[error("application catalog is unavailable or invalid")]
    CatalogUnavailable,
    #[error("runtime persistence is unavailable or invalid")]
    PersistenceUnavailable,
    #[error("runtime transition is invalid")]
    InvalidTransition,
}

impl From<RunpodRuntimeProviderError> for RunpodRuntimeError {
    fn from(_: RunpodRuntimeProviderError) -> Self {
        Self::ProviderUnavailable
    }
}

impl From<RunpodRuntimeCatalogError> for RunpodRuntimeError {
    fn from(_: RunpodRuntimeCatalogError) -> Self {
        Self::CatalogUnavailable
    }
}

impl From<RunpodRuntimeRepositoryError> for RunpodRuntimeError {
    fn from(_: RunpodRuntimeRepositoryError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<LifecycleOperationRepositoryError> for RunpodRuntimeError {
    fn from(_: LifecycleOperationRepositoryError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<SecretStoreError> for RunpodRuntimeError {
    fn from(_: SecretStoreError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<LifecycleError> for RunpodRuntimeError {
    fn from(error: LifecycleError) -> Self {
        match error {
            LifecycleError::InvalidTransition => Self::InvalidTransition,
            LifecycleError::OperationAlreadyRunning => Self::OperationInProgress,
        }
    }
}
