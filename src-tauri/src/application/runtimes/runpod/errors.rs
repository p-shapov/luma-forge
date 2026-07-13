use crate::application::{
    runtimes::{
        ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
        RuntimeOperationError,
    },
    secrets::SecretStoreError,
};

use super::ports::{RunpodRuntimeCatalogError, RunpodRuntimeProviderError};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
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
    #[error("runtime provider rejected the credential")]
    InvalidCredential,
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
    fn from(error: RunpodRuntimeProviderError) -> Self {
        match error {
            RunpodRuntimeProviderError::Unauthorized => Self::InvalidCredential,
            RunpodRuntimeProviderError::Unavailable
            | RunpodRuntimeProviderError::ProvisionerFailed => Self::ProviderUnavailable,
        }
    }
}

impl From<RunpodRuntimeCatalogError> for RunpodRuntimeError {
    fn from(_: RunpodRuntimeCatalogError) -> Self {
        Self::CatalogUnavailable
    }
}

impl From<RuntimePersistenceError> for RunpodRuntimeError {
    fn from(error: RuntimePersistenceError) -> Self {
        match error {
            RuntimePersistenceError::AlreadyExists => Self::AlreadyProvisioned,
            RuntimePersistenceError::OperationAlreadyRunning => Self::OperationInProgress,
            RuntimePersistenceError::NotFound
            | RuntimePersistenceError::Unavailable
            | RuntimePersistenceError::CorruptData => Self::PersistenceUnavailable,
        }
    }
}

impl From<RuntimeOperationRepositoryError> for RunpodRuntimeError {
    fn from(_: RuntimeOperationRepositoryError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<SecretStoreError> for RunpodRuntimeError {
    fn from(_: SecretStoreError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<RuntimeOperationError> for RunpodRuntimeError {
    fn from(_: RuntimeOperationError) -> Self {
        Self::InvalidTransition
    }
}

#[cfg(test)]
mod tests {
    use super::{RunpodRuntimeError, RunpodRuntimeProviderError};

    #[test]
    fn provider_errors_preserve_invalid_credentials() {
        assert_eq!(
            RunpodRuntimeError::from(RunpodRuntimeProviderError::Unauthorized),
            RunpodRuntimeError::InvalidCredential
        );
        assert_eq!(
            RunpodRuntimeError::from(RunpodRuntimeProviderError::Unavailable),
            RunpodRuntimeError::ProviderUnavailable
        );
        assert_eq!(
            RunpodRuntimeError::from(RunpodRuntimeProviderError::ProvisionerFailed),
            RunpodRuntimeError::ProviderUnavailable
        );
    }
}
