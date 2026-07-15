use serde::{Deserialize, Serialize};

use crate::application::{
    runtimes::{ports::RuntimeOperationRepositoryError, RuntimeError},
    secrets::SecretsError,
    workspace::WorkspaceError,
};

use super::models::{FacadeMappingError, InvalidPagination};

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct CommandError<Code: crate::diagnostics::DiagnosticValue> {
    #[diagnostic(show)]
    pub code: Code,
    #[diagnostic(show)]
    pub trace_id: String,
}

pub type CommandResult<T, Code> = Result<T, CommandError<Code>>;

fn error<Code: crate::diagnostics::DiagnosticValue>(code: Code) -> CommandError<Code> {
    CommandError {
        code,
        trace_id: crate::diagnostics::current_trace_uuid()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "trace-unavailable".to_owned()),
    }
}

macro_rules! error_code {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(
            crate::diagnostics::DiagnosticDebug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Serialize,
            Deserialize,
            specta::Type,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

error_code!(GetWorkflowsErrorCode {
    InvalidPagination,
    CatalogUnavailable,
    CommandError,
});
error_code!(GetWorkspacesErrorCode {
    InvalidPagination,
    PersistenceUnavailable,
    CommandError,
});
error_code!(CreateWorkspaceErrorCode {
    WorkflowNotFound,
    WorkspaceAlreadyExists,
    CatalogUnavailable,
    PersistenceUnavailable,
    CommandError,
});
error_code!(DeleteWorkspaceErrorCode {
    WorkspaceNotFound,
    RuntimeAttached,
    OperationRunning,
    PersistenceUnavailable,
    CommandError,
});
error_code!(ProvisionWorkspaceErrorCode {
    WorkspaceNotFound,
    WorkflowNotFound,
    AlreadyProvisioned,
    RuntimeFailed,
    OperationInProgress,
    CredentialMissing,
    CatalogUnavailable,
    PersistenceUnavailable,
    InvalidTransition,
    CommandError,
});
error_code!(CleanupWorkspaceErrorCode {
    WorkspaceNotFound,
    NotProvisioned,
    OperationInProgress,
    CredentialMissing,
    PersistenceUnavailable,
    InvalidTransition,
    CommandError,
});
error_code!(GetRuntimeOperationsErrorCode {
    InvalidPagination,
    PersistenceUnavailable,
    CommandError,
});
error_code!(GetRunpodPlacementErrorCode {
    CredentialMissing,
    InvalidCredential,
    ProviderUnavailable,
    CommandError,
});
error_code!(SetupApiKeyErrorCode {
    AlreadyConfigured,
    InvalidCredential,
    IdentityUnavailable,
    StorageUnavailable,
    CommandError,
});
error_code!(GetIdentityErrorCode {
    NotConfigured,
    InvalidCredential,
    IdentityUnavailable,
    StorageUnavailable,
    CommandError,
});
error_code!(DeleteApiKeyErrorCode {
    NotConfigured,
    StorageUnavailable,
    CommandError,
});

macro_rules! map_invalid_pagination {
    ($($code:ty),+ $(,)?) => {$ (
        impl From<InvalidPagination> for CommandError<$code> {
            fn from(_: InvalidPagination) -> Self {
                error(<$code>::InvalidPagination)
            }
        }
    )+};
}

map_invalid_pagination!(
    GetWorkflowsErrorCode,
    GetWorkspacesErrorCode,
    GetRuntimeOperationsErrorCode,
);

macro_rules! map_facade_error {
    ($($code:ty),+ $(,)?) => {$ (
        impl From<FacadeMappingError> for CommandError<$code> {
            fn from(_: FacadeMappingError) -> Self {
                error(<$code>::CommandError)
            }
        }
    )+};
}

map_facade_error!(
    GetWorkflowsErrorCode,
    GetWorkspacesErrorCode,
    CreateWorkspaceErrorCode,
    DeleteWorkspaceErrorCode,
    ProvisionWorkspaceErrorCode,
    CleanupWorkspaceErrorCode,
    GetRuntimeOperationsErrorCode,
    GetRunpodPlacementErrorCode,
    SetupApiKeyErrorCode,
    GetIdentityErrorCode,
    DeleteApiKeyErrorCode,
);

impl From<WorkspaceError> for CommandError<GetWorkflowsErrorCode> {
    fn from(value: WorkspaceError) -> Self {
        error(match value {
            WorkspaceError::CatalogUnavailable => GetWorkflowsErrorCode::CatalogUnavailable,
            WorkspaceError::NotFound
            | WorkspaceError::AlreadyExists
            | WorkspaceError::WorkflowNotFound
            | WorkspaceError::RuntimeAttached
            | WorkspaceError::OperationRunning
            | WorkspaceError::PersistenceUnavailable => GetWorkflowsErrorCode::CommandError,
        })
    }
}

impl From<WorkspaceError> for CommandError<GetWorkspacesErrorCode> {
    fn from(value: WorkspaceError) -> Self {
        error(match value {
            WorkspaceError::PersistenceUnavailable => {
                GetWorkspacesErrorCode::PersistenceUnavailable
            }
            WorkspaceError::NotFound
            | WorkspaceError::AlreadyExists
            | WorkspaceError::WorkflowNotFound
            | WorkspaceError::RuntimeAttached
            | WorkspaceError::OperationRunning
            | WorkspaceError::CatalogUnavailable => GetWorkspacesErrorCode::CommandError,
        })
    }
}

impl From<WorkspaceError> for CommandError<CreateWorkspaceErrorCode> {
    fn from(value: WorkspaceError) -> Self {
        error(match value {
            WorkspaceError::WorkflowNotFound => CreateWorkspaceErrorCode::WorkflowNotFound,
            WorkspaceError::AlreadyExists => CreateWorkspaceErrorCode::WorkspaceAlreadyExists,
            WorkspaceError::CatalogUnavailable => CreateWorkspaceErrorCode::CatalogUnavailable,
            WorkspaceError::PersistenceUnavailable => {
                CreateWorkspaceErrorCode::PersistenceUnavailable
            }
            WorkspaceError::NotFound
            | WorkspaceError::RuntimeAttached
            | WorkspaceError::OperationRunning => CreateWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<WorkspaceError> for CommandError<DeleteWorkspaceErrorCode> {
    fn from(value: WorkspaceError) -> Self {
        error(match value {
            WorkspaceError::NotFound => DeleteWorkspaceErrorCode::WorkspaceNotFound,
            WorkspaceError::RuntimeAttached => DeleteWorkspaceErrorCode::RuntimeAttached,
            WorkspaceError::OperationRunning => DeleteWorkspaceErrorCode::OperationRunning,
            WorkspaceError::PersistenceUnavailable => {
                DeleteWorkspaceErrorCode::PersistenceUnavailable
            }
            WorkspaceError::AlreadyExists
            | WorkspaceError::WorkflowNotFound
            | WorkspaceError::CatalogUnavailable => DeleteWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<WorkspaceError> for CommandError<CleanupWorkspaceErrorCode> {
    fn from(value: WorkspaceError) -> Self {
        error(match value {
            WorkspaceError::NotFound => CleanupWorkspaceErrorCode::WorkspaceNotFound,
            WorkspaceError::PersistenceUnavailable => {
                CleanupWorkspaceErrorCode::PersistenceUnavailable
            }
            WorkspaceError::AlreadyExists
            | WorkspaceError::WorkflowNotFound
            | WorkspaceError::RuntimeAttached
            | WorkspaceError::OperationRunning
            | WorkspaceError::CatalogUnavailable => CleanupWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<RuntimeError> for CommandError<ProvisionWorkspaceErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::WorkspaceNotFound => ProvisionWorkspaceErrorCode::WorkspaceNotFound,
            RuntimeError::WorkflowNotFound => ProvisionWorkspaceErrorCode::WorkflowNotFound,
            RuntimeError::AlreadyProvisioned => ProvisionWorkspaceErrorCode::AlreadyProvisioned,
            RuntimeError::RuntimeFailed => ProvisionWorkspaceErrorCode::RuntimeFailed,
            RuntimeError::OperationInProgress => ProvisionWorkspaceErrorCode::OperationInProgress,
            RuntimeError::CredentialMissing => ProvisionWorkspaceErrorCode::CredentialMissing,
            RuntimeError::CatalogUnavailable => ProvisionWorkspaceErrorCode::CatalogUnavailable,
            RuntimeError::PersistenceUnavailable => {
                ProvisionWorkspaceErrorCode::PersistenceUnavailable
            }
            RuntimeError::InvalidTransition => ProvisionWorkspaceErrorCode::InvalidTransition,
            RuntimeError::NotProvisioned
            | RuntimeError::InvalidCredential
            | RuntimeError::ProviderUnavailable => ProvisionWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<RuntimeError> for CommandError<CleanupWorkspaceErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::WorkspaceNotFound => CleanupWorkspaceErrorCode::WorkspaceNotFound,
            RuntimeError::NotProvisioned => CleanupWorkspaceErrorCode::NotProvisioned,
            RuntimeError::OperationInProgress => CleanupWorkspaceErrorCode::OperationInProgress,
            RuntimeError::CredentialMissing => CleanupWorkspaceErrorCode::CredentialMissing,
            RuntimeError::PersistenceUnavailable => {
                CleanupWorkspaceErrorCode::PersistenceUnavailable
            }
            RuntimeError::InvalidTransition => CleanupWorkspaceErrorCode::InvalidTransition,
            RuntimeError::WorkflowNotFound
            | RuntimeError::AlreadyProvisioned
            | RuntimeError::RuntimeFailed
            | RuntimeError::InvalidCredential
            | RuntimeError::ProviderUnavailable
            | RuntimeError::CatalogUnavailable => CleanupWorkspaceErrorCode::CommandError,
        })
    }
}

impl From<RuntimeOperationRepositoryError> for CommandError<GetRuntimeOperationsErrorCode> {
    fn from(value: RuntimeOperationRepositoryError) -> Self {
        error(match value {
            RuntimeOperationRepositoryError::Unavailable
            | RuntimeOperationRepositoryError::CorruptData => {
                GetRuntimeOperationsErrorCode::PersistenceUnavailable
            }
        })
    }
}

impl From<RuntimeError> for CommandError<GetRunpodPlacementErrorCode> {
    fn from(value: RuntimeError) -> Self {
        error(match value {
            RuntimeError::CredentialMissing => GetRunpodPlacementErrorCode::CredentialMissing,
            RuntimeError::InvalidCredential => GetRunpodPlacementErrorCode::InvalidCredential,
            RuntimeError::ProviderUnavailable => GetRunpodPlacementErrorCode::ProviderUnavailable,
            RuntimeError::WorkspaceNotFound
            | RuntimeError::WorkflowNotFound
            | RuntimeError::AlreadyProvisioned
            | RuntimeError::RuntimeFailed
            | RuntimeError::OperationInProgress
            | RuntimeError::NotProvisioned
            | RuntimeError::CatalogUnavailable
            | RuntimeError::PersistenceUnavailable
            | RuntimeError::InvalidTransition => GetRunpodPlacementErrorCode::CommandError,
        })
    }
}

impl From<SecretsError> for CommandError<SetupApiKeyErrorCode> {
    fn from(value: SecretsError) -> Self {
        error(match value {
            SecretsError::AlreadyConfigured => SetupApiKeyErrorCode::AlreadyConfigured,
            SecretsError::InvalidCredential => SetupApiKeyErrorCode::InvalidCredential,
            SecretsError::IdentityUnavailable => SetupApiKeyErrorCode::IdentityUnavailable,
            SecretsError::StorageUnavailable => SetupApiKeyErrorCode::StorageUnavailable,
            SecretsError::NotConfigured => SetupApiKeyErrorCode::CommandError,
        })
    }
}

impl From<SecretsError> for CommandError<GetIdentityErrorCode> {
    fn from(value: SecretsError) -> Self {
        error(match value {
            SecretsError::NotConfigured => GetIdentityErrorCode::NotConfigured,
            SecretsError::InvalidCredential => GetIdentityErrorCode::InvalidCredential,
            SecretsError::IdentityUnavailable => GetIdentityErrorCode::IdentityUnavailable,
            SecretsError::StorageUnavailable => GetIdentityErrorCode::StorageUnavailable,
            SecretsError::AlreadyConfigured => GetIdentityErrorCode::CommandError,
        })
    }
}

impl From<SecretsError> for CommandError<DeleteApiKeyErrorCode> {
    fn from(value: SecretsError) -> Self {
        error(match value {
            SecretsError::NotConfigured => DeleteApiKeyErrorCode::NotConfigured,
            SecretsError::StorageUnavailable => DeleteApiKeyErrorCode::StorageUnavailable,
            SecretsError::AlreadyConfigured
            | SecretsError::InvalidCredential
            | SecretsError::IdentityUnavailable => DeleteApiKeyErrorCode::CommandError,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_errors_map_to_command_specific_codes() {
        let create: CommandError<CreateWorkspaceErrorCode> = WorkspaceError::AlreadyExists.into();
        let delete: CommandError<DeleteWorkspaceErrorCode> = WorkspaceError::RuntimeAttached.into();
        let cleanup: CommandError<CleanupWorkspaceErrorCode> = WorkspaceError::NotFound.into();

        assert_eq!(
            create.code,
            CreateWorkspaceErrorCode::WorkspaceAlreadyExists
        );
        assert_eq!(delete.code, DeleteWorkspaceErrorCode::RuntimeAttached);
        assert_eq!(cleanup.code, CleanupWorkspaceErrorCode::WorkspaceNotFound);
    }

    #[test]
    fn unexpected_runtime_errors_map_to_command_error() {
        let provision: CommandError<ProvisionWorkspaceErrorCode> =
            RuntimeError::ProviderUnavailable.into();
        let cleanup: CommandError<CleanupWorkspaceErrorCode> =
            RuntimeError::InvalidCredential.into();

        assert_eq!(provision.code, ProvisionWorkspaceErrorCode::CommandError);
        assert_eq!(cleanup.code, CleanupWorkspaceErrorCode::CommandError);
    }

    #[test]
    fn secret_errors_map_to_each_command_surface() {
        let setup: CommandError<SetupApiKeyErrorCode> = SecretsError::AlreadyConfigured.into();
        let identity: CommandError<GetIdentityErrorCode> = SecretsError::NotConfigured.into();
        let delete: CommandError<DeleteApiKeyErrorCode> = SecretsError::NotConfigured.into();

        assert_eq!(setup.code, SetupApiKeyErrorCode::AlreadyConfigured);
        assert_eq!(identity.code, GetIdentityErrorCode::NotConfigured);
        assert_eq!(delete.code, DeleteApiKeyErrorCode::NotConfigured);
    }
}
