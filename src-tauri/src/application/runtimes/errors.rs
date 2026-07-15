use crate::application::secrets::SecretStoreError;

use super::ports::{RuntimeOperationRepositoryError, RuntimePersistenceError};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeOperationError {
    #[error("runtime operation transition is invalid")]
    InvalidTransition,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
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

impl From<RuntimePersistenceError> for RuntimeError {
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

impl From<RuntimeOperationRepositoryError> for RuntimeError {
    fn from(_: RuntimeOperationRepositoryError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<SecretStoreError> for RuntimeError {
    fn from(_: SecretStoreError) -> Self {
        Self::PersistenceUnavailable
    }
}

impl From<RuntimeOperationError> for RuntimeError {
    fn from(_: RuntimeOperationError) -> Self {
        Self::InvalidTransition
    }
}

#[cfg(test)]
mod tests {
    use crate::application::{
        runtimes::ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
        secrets::SecretStoreError,
    };

    use super::{RuntimeError, RuntimeOperationError};

    #[test]
    fn shared_persistence_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::AlreadyExists),
            RuntimeError::AlreadyProvisioned
        );
        assert_eq!(
            RuntimeError::from(RuntimePersistenceError::OperationAlreadyRunning),
            RuntimeError::OperationInProgress
        );
        for error in [
            RuntimePersistenceError::NotFound,
            RuntimePersistenceError::Unavailable,
            RuntimePersistenceError::CorruptData,
        ] {
            assert_eq!(
                RuntimeError::from(error),
                RuntimeError::PersistenceUnavailable
            );
        }
    }

    #[test]
    fn shared_operation_secret_and_transition_errors_map_to_runtime_categories() {
        assert_eq!(
            RuntimeError::from(RuntimeOperationRepositoryError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(SecretStoreError::Unavailable),
            RuntimeError::PersistenceUnavailable
        );
        assert_eq!(
            RuntimeError::from(RuntimeOperationError::InvalidTransition),
            RuntimeError::InvalidTransition
        );
    }
}
