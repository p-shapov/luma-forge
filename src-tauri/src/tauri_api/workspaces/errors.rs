use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    lifecycle_journal::LifecycleJournalError, provider::errors::ProviderApiError,
    runtime_catalog::RuntimeCatalogError, secrets::SecretsStorageError,
    workflow_catalog::WorkflowCatalogError, workspace::WorkspaceError,
    workspace_catalog::WorkspaceCatalogError,
};

macro_rules! define_workspace_error_code {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            #[error("native initialization failed")]
            NativeInitializationFailed,
            #[error("provider request was unauthorized")]
            ProviderUnauthorized,
            #[error("provider request has insufficient permissions")]
            ProviderInsufficientPermissions,
            #[error("provider request was rate limited")]
            ProviderRateLimited,
            #[error("provider request timed out")]
            ProviderTimeout,
            #[error("provider request failed")]
            ProviderRequestFailed,
            #[error("workflow catalog parse failed")]
            WorkflowCatalogParseFailed,
            #[error("workflow catalog validation failed")]
            WorkflowCatalogValidationFailed,
            #[error("runtime catalog parse failed")]
            RuntimeCatalogParseFailed,
            #[error("runtime catalog validation failed")]
            RuntimeCatalogValidationFailed,
            #[error("workspace catalog storage unavailable")]
            WorkspaceCatalogStorageUnavailable,
            #[error("workspace catalog schema is invalid")]
            WorkspaceCatalogSchemaInvalid,
            #[error("workspace catalog data is invalid")]
            WorkspaceCatalogDataInvalid,
            #[error("lifecycle journal storage unavailable")]
            LifecycleJournalStorageUnavailable,
            #[error("lifecycle journal schema is invalid")]
            LifecycleJournalSchemaInvalid,
            #[error("lifecycle journal data is invalid")]
            LifecycleJournalDataInvalid,
            #[error("workspace already exists")]
            WorkspaceAlreadyExists,
            #[error("workspace was not found")]
            WorkspaceNotFound,
            #[error("api key is not configured")]
            KeyNotFound,
            #[error("secure storage is unavailable")]
            StoreUnavailable,
            #[error("workspace already has a running lifecycle operation")]
            LifecycleOperationAlreadyRunning,
            #[error("invalid runtime state")]
            InvalidRuntimeState,
        }
    };
}

define_workspace_error_code!(CreateRunpodWorkspaceErrorCode);
define_workspace_error_code!(ProvisionWorkspaceErrorCode);
define_workspace_error_code!(CleanupWorkspaceErrorCode);
define_workspace_error_code!(DeleteWorkspaceErrorCode);
define_workspace_error_code!(GetRunningLifecycleOperationsErrorCode);
define_workspace_error_code!(GetLatestLifecycleOperationErrorCode);

pub fn create_runpod_workspace_error(error: &WorkspaceError) -> CreateRunpodWorkspaceErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => {
            CreateRunpodWorkspaceErrorCode::ProviderUnauthorized
        }
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            CreateRunpodWorkspaceErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => {
            CreateRunpodWorkspaceErrorCode::ProviderRateLimited
        }
        WorkspaceErrorKind::ProviderTimeout => CreateRunpodWorkspaceErrorCode::ProviderTimeout,
        WorkspaceErrorKind::ProviderRequestFailed => {
            CreateRunpodWorkspaceErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            CreateRunpodWorkspaceErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            CreateRunpodWorkspaceErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            CreateRunpodWorkspaceErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            CreateRunpodWorkspaceErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            CreateRunpodWorkspaceErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            CreateRunpodWorkspaceErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            CreateRunpodWorkspaceErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            CreateRunpodWorkspaceErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            CreateRunpodWorkspaceErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            CreateRunpodWorkspaceErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            CreateRunpodWorkspaceErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => CreateRunpodWorkspaceErrorCode::WorkspaceNotFound,
        WorkspaceErrorKind::KeyNotFound => CreateRunpodWorkspaceErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => CreateRunpodWorkspaceErrorCode::StoreUnavailable,
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            CreateRunpodWorkspaceErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => {
            CreateRunpodWorkspaceErrorCode::InvalidRuntimeState
        }
    }
}

pub fn provision_workspace_error(error: &WorkspaceError) -> ProvisionWorkspaceErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => {
            ProvisionWorkspaceErrorCode::ProviderUnauthorized
        }
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            ProvisionWorkspaceErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => ProvisionWorkspaceErrorCode::ProviderRateLimited,
        WorkspaceErrorKind::ProviderTimeout => ProvisionWorkspaceErrorCode::ProviderTimeout,
        WorkspaceErrorKind::ProviderRequestFailed => {
            ProvisionWorkspaceErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            ProvisionWorkspaceErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            ProvisionWorkspaceErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            ProvisionWorkspaceErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            ProvisionWorkspaceErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            ProvisionWorkspaceErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            ProvisionWorkspaceErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            ProvisionWorkspaceErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            ProvisionWorkspaceErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            ProvisionWorkspaceErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            ProvisionWorkspaceErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            ProvisionWorkspaceErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => ProvisionWorkspaceErrorCode::WorkspaceNotFound,
        WorkspaceErrorKind::KeyNotFound => ProvisionWorkspaceErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => ProvisionWorkspaceErrorCode::StoreUnavailable,
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            ProvisionWorkspaceErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => ProvisionWorkspaceErrorCode::InvalidRuntimeState,
    }
}

pub fn cleanup_workspace_error(error: &WorkspaceError) -> CleanupWorkspaceErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => CleanupWorkspaceErrorCode::ProviderUnauthorized,
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            CleanupWorkspaceErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => CleanupWorkspaceErrorCode::ProviderRateLimited,
        WorkspaceErrorKind::ProviderTimeout => CleanupWorkspaceErrorCode::ProviderTimeout,
        WorkspaceErrorKind::ProviderRequestFailed => {
            CleanupWorkspaceErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            CleanupWorkspaceErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            CleanupWorkspaceErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            CleanupWorkspaceErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            CleanupWorkspaceErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            CleanupWorkspaceErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            CleanupWorkspaceErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            CleanupWorkspaceErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            CleanupWorkspaceErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            CleanupWorkspaceErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            CleanupWorkspaceErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            CleanupWorkspaceErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => CleanupWorkspaceErrorCode::WorkspaceNotFound,
        WorkspaceErrorKind::KeyNotFound => CleanupWorkspaceErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => CleanupWorkspaceErrorCode::StoreUnavailable,
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            CleanupWorkspaceErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => CleanupWorkspaceErrorCode::InvalidRuntimeState,
    }
}

pub fn delete_workspace_error(error: &WorkspaceError) -> DeleteWorkspaceErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => DeleteWorkspaceErrorCode::ProviderUnauthorized,
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            DeleteWorkspaceErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => DeleteWorkspaceErrorCode::ProviderRateLimited,
        WorkspaceErrorKind::ProviderTimeout => DeleteWorkspaceErrorCode::ProviderTimeout,
        WorkspaceErrorKind::ProviderRequestFailed => {
            DeleteWorkspaceErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            DeleteWorkspaceErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            DeleteWorkspaceErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            DeleteWorkspaceErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            DeleteWorkspaceErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            DeleteWorkspaceErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            DeleteWorkspaceErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            DeleteWorkspaceErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            DeleteWorkspaceErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            DeleteWorkspaceErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            DeleteWorkspaceErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            DeleteWorkspaceErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => DeleteWorkspaceErrorCode::WorkspaceNotFound,
        WorkspaceErrorKind::KeyNotFound => DeleteWorkspaceErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => DeleteWorkspaceErrorCode::StoreUnavailable,
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            DeleteWorkspaceErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => DeleteWorkspaceErrorCode::InvalidRuntimeState,
    }
}

pub fn get_running_lifecycle_operations_error(
    error: &WorkspaceError,
) -> GetRunningLifecycleOperationsErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => {
            GetRunningLifecycleOperationsErrorCode::ProviderUnauthorized
        }
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            GetRunningLifecycleOperationsErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => {
            GetRunningLifecycleOperationsErrorCode::ProviderRateLimited
        }
        WorkspaceErrorKind::ProviderTimeout => {
            GetRunningLifecycleOperationsErrorCode::ProviderTimeout
        }
        WorkspaceErrorKind::ProviderRequestFailed => {
            GetRunningLifecycleOperationsErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            GetRunningLifecycleOperationsErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            GetRunningLifecycleOperationsErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            GetRunningLifecycleOperationsErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            GetRunningLifecycleOperationsErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            GetRunningLifecycleOperationsErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            GetRunningLifecycleOperationsErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            GetRunningLifecycleOperationsErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            GetRunningLifecycleOperationsErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            GetRunningLifecycleOperationsErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            GetRunningLifecycleOperationsErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            GetRunningLifecycleOperationsErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => {
            GetRunningLifecycleOperationsErrorCode::WorkspaceNotFound
        }
        WorkspaceErrorKind::KeyNotFound => GetRunningLifecycleOperationsErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => {
            GetRunningLifecycleOperationsErrorCode::StoreUnavailable
        }
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            GetRunningLifecycleOperationsErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => {
            GetRunningLifecycleOperationsErrorCode::InvalidRuntimeState
        }
    }
}

pub fn get_latest_lifecycle_operation_error(
    error: &WorkspaceError,
) -> GetLatestLifecycleOperationErrorCode {
    match workspace_error_kind(error) {
        WorkspaceErrorKind::ProviderUnauthorized => {
            GetLatestLifecycleOperationErrorCode::ProviderUnauthorized
        }
        WorkspaceErrorKind::ProviderInsufficientPermissions => {
            GetLatestLifecycleOperationErrorCode::ProviderInsufficientPermissions
        }
        WorkspaceErrorKind::ProviderRateLimited => {
            GetLatestLifecycleOperationErrorCode::ProviderRateLimited
        }
        WorkspaceErrorKind::ProviderTimeout => {
            GetLatestLifecycleOperationErrorCode::ProviderTimeout
        }
        WorkspaceErrorKind::ProviderRequestFailed => {
            GetLatestLifecycleOperationErrorCode::ProviderRequestFailed
        }
        WorkspaceErrorKind::WorkflowCatalogParseFailed => {
            GetLatestLifecycleOperationErrorCode::WorkflowCatalogParseFailed
        }
        WorkspaceErrorKind::WorkflowCatalogValidationFailed => {
            GetLatestLifecycleOperationErrorCode::WorkflowCatalogValidationFailed
        }
        WorkspaceErrorKind::RuntimeCatalogParseFailed => {
            GetLatestLifecycleOperationErrorCode::RuntimeCatalogParseFailed
        }
        WorkspaceErrorKind::RuntimeCatalogValidationFailed => {
            GetLatestLifecycleOperationErrorCode::RuntimeCatalogValidationFailed
        }
        WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable => {
            GetLatestLifecycleOperationErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid => {
            GetLatestLifecycleOperationErrorCode::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceErrorKind::WorkspaceCatalogDataInvalid => {
            GetLatestLifecycleOperationErrorCode::WorkspaceCatalogDataInvalid
        }
        WorkspaceErrorKind::LifecycleJournalStorageUnavailable => {
            GetLatestLifecycleOperationErrorCode::LifecycleJournalStorageUnavailable
        }
        WorkspaceErrorKind::LifecycleJournalSchemaInvalid => {
            GetLatestLifecycleOperationErrorCode::LifecycleJournalSchemaInvalid
        }
        WorkspaceErrorKind::LifecycleJournalDataInvalid => {
            GetLatestLifecycleOperationErrorCode::LifecycleJournalDataInvalid
        }
        WorkspaceErrorKind::WorkspaceAlreadyExists => {
            GetLatestLifecycleOperationErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceErrorKind::WorkspaceNotFound => {
            GetLatestLifecycleOperationErrorCode::WorkspaceNotFound
        }
        WorkspaceErrorKind::KeyNotFound => GetLatestLifecycleOperationErrorCode::KeyNotFound,
        WorkspaceErrorKind::StoreUnavailable => {
            GetLatestLifecycleOperationErrorCode::StoreUnavailable
        }
        WorkspaceErrorKind::LifecycleOperationAlreadyRunning => {
            GetLatestLifecycleOperationErrorCode::LifecycleOperationAlreadyRunning
        }
        WorkspaceErrorKind::InvalidRuntimeState => {
            GetLatestLifecycleOperationErrorCode::InvalidRuntimeState
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceErrorKind {
    ProviderUnauthorized,
    ProviderInsufficientPermissions,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed,
    WorkflowCatalogParseFailed,
    WorkflowCatalogValidationFailed,
    RuntimeCatalogParseFailed,
    RuntimeCatalogValidationFailed,
    WorkspaceCatalogStorageUnavailable,
    WorkspaceCatalogSchemaInvalid,
    WorkspaceCatalogDataInvalid,
    LifecycleJournalStorageUnavailable,
    LifecycleJournalSchemaInvalid,
    LifecycleJournalDataInvalid,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
    KeyNotFound,
    StoreUnavailable,
    LifecycleOperationAlreadyRunning,
    InvalidRuntimeState,
}

fn workspace_error_kind(error: &WorkspaceError) -> WorkspaceErrorKind {
    match error {
        WorkspaceError::ProviderApiError(error) => provider_error(error),
        WorkspaceError::RuntimeProviderApiKeyUnavailable(error)
        | WorkspaceError::WorkflowProviderApiKeyUnavailable(error) => secret_error(error),
        WorkspaceError::WorkflowCatalogInvalid(error) => workflow_catalog_error(error),
        WorkspaceError::RuntimeCatalogInvalid(error) => runtime_catalog_error(error),
        WorkspaceError::WorkspaceCatalogInvalid(error) => workspace_catalog_error(error),
        WorkspaceError::LifecycleJournalInvalid(error) => lifecycle_journal_error(error),
        WorkspaceError::WorkspaceNotFound { .. } => WorkspaceErrorKind::WorkspaceNotFound,
        WorkspaceError::LifecycleOperationAlreadyRunning { .. } => {
            WorkspaceErrorKind::LifecycleOperationAlreadyRunning
        }
        WorkspaceError::InvalidState { .. } => WorkspaceErrorKind::InvalidRuntimeState,
    }
}

fn provider_error(error: &ProviderApiError) -> WorkspaceErrorKind {
    match error {
        ProviderApiError::Unauthorized => WorkspaceErrorKind::ProviderUnauthorized,
        ProviderApiError::InsufficientPermissions => {
            WorkspaceErrorKind::ProviderInsufficientPermissions
        }
        ProviderApiError::RateLimited => WorkspaceErrorKind::ProviderRateLimited,
        ProviderApiError::Timeout => WorkspaceErrorKind::ProviderTimeout,
        ProviderApiError::RequestFailed { .. } => WorkspaceErrorKind::ProviderRequestFailed,
    }
}

fn secret_error(error: &SecretsStorageError) -> WorkspaceErrorKind {
    match error {
        SecretsStorageError::SecretRequired | SecretsStorageError::KeyNotFound => {
            WorkspaceErrorKind::KeyNotFound
        }
        SecretsStorageError::KeyAlreadyExists
        | SecretsStorageError::StoreUnavailable
        | SecretsStorageError::StoredSecretInvalid
        | SecretsStorageError::IdentityRequestFailed(_)
        | SecretsStorageError::IdentityResponseInvalid { .. } => {
            WorkspaceErrorKind::StoreUnavailable
        }
    }
}

fn workflow_catalog_error(error: &WorkflowCatalogError) -> WorkspaceErrorKind {
    match error {
        WorkflowCatalogError::ParseFailed { .. } => WorkspaceErrorKind::WorkflowCatalogParseFailed,
        WorkflowCatalogError::ValidationFailed { .. } => {
            WorkspaceErrorKind::WorkflowCatalogValidationFailed
        }
    }
}

fn runtime_catalog_error(error: &RuntimeCatalogError) -> WorkspaceErrorKind {
    match error {
        RuntimeCatalogError::ParseFailed { .. } => WorkspaceErrorKind::RuntimeCatalogParseFailed,
        RuntimeCatalogError::ValidationFailed { .. } => {
            WorkspaceErrorKind::RuntimeCatalogValidationFailed
        }
    }
}

fn workspace_catalog_error(error: &WorkspaceCatalogError) -> WorkspaceErrorKind {
    match error {
        WorkspaceCatalogError::StorageUnavailable { .. } => {
            WorkspaceErrorKind::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceCatalogError::SchemaInvalid { .. } => {
            WorkspaceErrorKind::WorkspaceCatalogSchemaInvalid
        }
        WorkspaceCatalogError::DataInvalid { .. } => {
            WorkspaceErrorKind::WorkspaceCatalogDataInvalid
        }
        WorkspaceCatalogError::WorkspaceAlreadyExists => WorkspaceErrorKind::WorkspaceAlreadyExists,
        WorkspaceCatalogError::WorkspaceNotFound => WorkspaceErrorKind::WorkspaceNotFound,
    }
}

fn lifecycle_journal_error(error: &LifecycleJournalError) -> WorkspaceErrorKind {
    match error {
        LifecycleJournalError::OperationNotFound => WorkspaceErrorKind::InvalidRuntimeState,
        LifecycleJournalError::RunningOperationExists { .. } => {
            WorkspaceErrorKind::LifecycleOperationAlreadyRunning
        }
        LifecycleJournalError::StorageUnavailable { .. } => {
            WorkspaceErrorKind::LifecycleJournalStorageUnavailable
        }
        LifecycleJournalError::SchemaInvalid { .. } => {
            WorkspaceErrorKind::LifecycleJournalSchemaInvalid
        }
        LifecycleJournalError::DataInvalid { .. } => {
            WorkspaceErrorKind::LifecycleJournalDataInvalid
        }
    }
}
