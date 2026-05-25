use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    hugging_face_setup::HuggingFaceSetupError, provider_setup::ProviderSetupError,
    workspace_provisioning::WorkspaceProvisioningError, workspace_removal::WorkspaceRemovalError,
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NativeCommandErrorCode {
    ProviderSetupIncomplete,
    ProviderSetupNotFound,
    ProviderSetupAlreadyExists,
    ProviderApiKeyRequired,
    ProviderApiKeyUnauthorized,
    StoredProviderApiKeyInvalid,
    ProviderApiUnavailable,
    ProviderRateLimited,
    ProviderRequestRejected,
    ProviderResponseInvalid,
    ProviderInventoryInvalid,
    ProviderIdentityResponseInvalid,
    SecureKeyringUnavailable,
    ProviderSetupRecoveryRequired,
    HuggingFaceSetupNotFound,
    HuggingFaceApiKeyRequired,
    HuggingFaceApiKeyUnauthorized,
    HuggingFaceApiKeyInsufficientPermissions,
    StoredHuggingFaceApiKeyInvalid,
    HuggingFaceApiUnavailable,
    HuggingFaceRateLimited,
    HuggingFaceIdentityResponseInvalid,
    WorkflowCatalogUnavailable,
    WorkspaceCatalogUnavailable,
    WorkspaceCatalogStorageUnavailable,
    WorkspaceCatalogMigrationFailed,
    WorkspaceCatalogQueryFailed,
    WorkspaceCatalogCorrupt,
    WorkspaceCatalogSchemaMismatch,
    PlacementProviderMismatch,
    PlacementDatacenterRequired,
    PlacementGpuRequired,
    WorkflowPresetStale,
    StorageSizeBelowPresetMinimum,
    EndpointKeepAliveOutOfRange,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
    InvalidWorkspaceLifecycle,
    InvalidWorkspaceId,
    WorkspaceNameRequired,
    InvalidWorkspaceMetadata,
    ProviderResourceNotFound,
    ProviderOrphanedResources,
    ProviderOperationConflict,
    ProviderOperationIndeterminate,
    HuggingFaceApiKeySetupRequired,
    CleanupFailed,
    ProvisionerWorkerTokenInvalid,
    ProvisionerWorkerUnauthorized,
    ProvisionerWorkerUnavailable,
    ProvisionerWorkerConflict,
    ProvisionerWorkerResponseInvalid,
    ProvisionerWorkerFailed,
}

impl NativeCommandErrorCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ProviderSetupIncomplete => "provider_setup_incomplete",
            Self::ProviderSetupNotFound => "provider_setup_not_found",
            Self::ProviderSetupAlreadyExists => "provider_setup_already_exists",
            Self::ProviderApiKeyRequired => "provider_api_key_required",
            Self::ProviderApiKeyUnauthorized => "provider_api_key_unauthorized",
            Self::StoredProviderApiKeyInvalid => "stored_provider_api_key_invalid",
            Self::ProviderApiUnavailable => "provider_api_unavailable",
            Self::ProviderRateLimited => "provider_rate_limited",
            Self::ProviderRequestRejected => "provider_request_rejected",
            Self::ProviderResponseInvalid => "provider_response_invalid",
            Self::ProviderInventoryInvalid => "provider_inventory_invalid",
            Self::ProviderIdentityResponseInvalid => "provider_identity_response_invalid",
            Self::SecureKeyringUnavailable => "secure_keyring_unavailable",
            Self::ProviderSetupRecoveryRequired => "provider_setup_recovery_required",
            Self::HuggingFaceSetupNotFound => "hugging_face_setup_not_found",
            Self::HuggingFaceApiKeyRequired => "hugging_face_api_key_required",
            Self::HuggingFaceApiKeyUnauthorized => "hugging_face_api_key_unauthorized",
            Self::HuggingFaceApiKeyInsufficientPermissions => {
                "hugging_face_api_key_insufficient_permissions"
            }
            Self::StoredHuggingFaceApiKeyInvalid => "stored_hugging_face_api_key_invalid",
            Self::HuggingFaceApiUnavailable => "hugging_face_api_unavailable",
            Self::HuggingFaceRateLimited => "hugging_face_rate_limited",
            Self::HuggingFaceIdentityResponseInvalid => "hugging_face_identity_response_invalid",
            Self::WorkflowCatalogUnavailable => "workflow_catalog_unavailable",
            Self::WorkspaceCatalogUnavailable => "workspace_catalog_unavailable",
            Self::WorkspaceCatalogStorageUnavailable => "workspace_catalog_storage_unavailable",
            Self::WorkspaceCatalogMigrationFailed => "workspace_catalog_migration_failed",
            Self::WorkspaceCatalogQueryFailed => "workspace_catalog_query_failed",
            Self::WorkspaceCatalogCorrupt => "workspace_catalog_corrupt",
            Self::WorkspaceCatalogSchemaMismatch => "workspace_catalog_schema_mismatch",
            Self::PlacementProviderMismatch => "placement_provider_mismatch",
            Self::PlacementDatacenterRequired => "placement_datacenter_required",
            Self::PlacementGpuRequired => "placement_gpu_required",
            Self::WorkflowPresetStale => "workflow_preset_stale",
            Self::StorageSizeBelowPresetMinimum => "storage_size_below_preset_minimum",
            Self::EndpointKeepAliveOutOfRange => "endpoint_keep_alive_out_of_range",
            Self::WorkspaceAlreadyExists => "workspace_already_exists",
            Self::WorkspaceNotFound => "workspace_not_found",
            Self::InvalidWorkspaceLifecycle => "invalid_workspace_lifecycle",
            Self::InvalidWorkspaceId => "invalid_workspace_id",
            Self::WorkspaceNameRequired => "workspace_name_required",
            Self::InvalidWorkspaceMetadata => "invalid_workspace_metadata",
            Self::ProviderResourceNotFound => "provider_resource_not_found",
            Self::ProviderOrphanedResources => "provider_orphaned_resources",
            Self::ProviderOperationConflict => "provider_operation_conflict",
            Self::ProviderOperationIndeterminate => "provider_operation_indeterminate",
            Self::HuggingFaceApiKeySetupRequired => "hugging_face_api_key_setup_required",
            Self::CleanupFailed => "cleanup_failed",
            Self::ProvisionerWorkerTokenInvalid => "provisioner_worker_token_invalid",
            Self::ProvisionerWorkerUnauthorized => "provisioner_worker_unauthorized",
            Self::ProvisionerWorkerUnavailable => "provisioner_worker_unavailable",
            Self::ProvisionerWorkerConflict => "provisioner_worker_conflict",
            Self::ProvisionerWorkerResponseInvalid => "provisioner_worker_response_invalid",
            Self::ProvisionerWorkerFailed => "provisioner_worker_failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NativeCommandError {
    pub code: NativeCommandErrorCode,
    pub message: String,
    pub retryable: bool,
    pub field: Option<String>,
    pub recovery_action: Option<String>,
}

impl From<ProviderSetupError> for NativeCommandError {
    fn from(error: ProviderSetupError) -> Self {
        Self {
            code: provider_setup_error_code(&error),
            message: provider_setup_error_message(&error).to_string(),
            retryable: provider_setup_error_retryable(&error),
            field: provider_setup_error_field(&error).map(str::to_string),
            recovery_action: provider_setup_error_recovery_action(&error).map(str::to_string),
        }
    }
}

impl From<WorkspaceSetupError> for NativeCommandError {
    fn from(error: WorkspaceSetupError) -> Self {
        Self {
            code: error_code(&error),
            message: error_message(&error).to_string(),
            retryable: error_retryable(&error),
            field: error_field(&error).map(str::to_string),
            recovery_action: error_recovery_action(&error).map(str::to_string),
        }
    }
}

impl From<HuggingFaceSetupError> for NativeCommandError {
    fn from(error: HuggingFaceSetupError) -> Self {
        Self {
            code: hugging_face_setup_error_code(&error),
            message: hugging_face_setup_error_message(&error).to_string(),
            retryable: hugging_face_setup_error_retryable(&error),
            field: hugging_face_setup_error_field(&error).map(str::to_string),
            recovery_action: hugging_face_setup_error_recovery_action(&error).map(str::to_string),
        }
    }
}

impl From<WorkspaceProvisioningError> for NativeCommandError {
    fn from(error: WorkspaceProvisioningError) -> Self {
        Self {
            code: provisioning_error_code(&error),
            message: provisioning_error_message(&error).to_string(),
            retryable: provisioning_error_retryable(&error),
            field: provisioning_error_field(&error).map(str::to_string),
            recovery_action: provisioning_error_recovery_action(&error).map(str::to_string),
        }
    }
}

impl From<WorkspaceRemovalError> for NativeCommandError {
    fn from(error: WorkspaceRemovalError) -> Self {
        let provisioning_error = match error {
            WorkspaceRemovalError::WorkspaceNotFound => {
                WorkspaceProvisioningError::WorkspaceNotFound
            }
            WorkspaceRemovalError::InvalidWorkspaceLifecycle => {
                WorkspaceProvisioningError::InvalidWorkspaceLifecycle
            }
            WorkspaceRemovalError::WorkspaceCatalogUnavailable => {
                WorkspaceProvisioningError::WorkspaceCatalogUnavailable
            }
            WorkspaceRemovalError::WorkspaceCatalogStorageUnavailable => {
                WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable
            }
            WorkspaceRemovalError::WorkspaceCatalogMigrationFailed => {
                WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed
            }
            WorkspaceRemovalError::WorkspaceCatalogQueryFailed => {
                WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
            }
            WorkspaceRemovalError::WorkspaceCatalogCorrupt => {
                WorkspaceProvisioningError::WorkspaceCatalogCorrupt
            }
            WorkspaceRemovalError::WorkspaceCatalogSchemaMismatch => {
                WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch
            }
            WorkspaceRemovalError::ProviderSetupIncomplete => {
                WorkspaceProvisioningError::ProviderSetupIncomplete
            }
            WorkspaceRemovalError::ProviderApiKeyUnauthorized => {
                WorkspaceProvisioningError::ProviderApiKeyUnauthorized
            }
            WorkspaceRemovalError::ProviderApiUnavailable => {
                WorkspaceProvisioningError::ProviderApiUnavailable
            }
            WorkspaceRemovalError::ProviderRateLimited => {
                WorkspaceProvisioningError::ProviderRateLimited
            }
            WorkspaceRemovalError::ProviderRequestRejected => {
                WorkspaceProvisioningError::ProviderRequestRejected
            }
            WorkspaceRemovalError::ProviderResponseInvalid => {
                WorkspaceProvisioningError::ProviderResponseInvalid
            }
            WorkspaceRemovalError::ProviderResourceNotFound => {
                WorkspaceProvisioningError::ProviderResourceNotFound
            }
            WorkspaceRemovalError::ProviderOrphanedResources => {
                WorkspaceProvisioningError::ProviderOrphanedResources
            }
            WorkspaceRemovalError::ProviderOperationConflict => {
                WorkspaceProvisioningError::ProviderOperationConflict
            }
            WorkspaceRemovalError::ProviderOperationIndeterminate => {
                WorkspaceProvisioningError::ProviderOperationIndeterminate
            }
            WorkspaceRemovalError::HuggingFaceApiKeySetupRequired => {
                WorkspaceProvisioningError::HuggingFaceApiKeySetupRequired
            }
            WorkspaceRemovalError::SecureKeyringUnavailable => {
                WorkspaceProvisioningError::SecureKeyringUnavailable
            }
            WorkspaceRemovalError::ProvisionerWorkerTokenInvalid => {
                WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid
            }
            WorkspaceRemovalError::CleanupFailed => WorkspaceProvisioningError::CleanupFailed,
        };
        NativeCommandError::from(provisioning_error)
    }
}

fn hugging_face_setup_error_code(error: &HuggingFaceSetupError) -> NativeCommandErrorCode {
    match error {
        HuggingFaceSetupError::HuggingFaceSetupNotFound => {
            NativeCommandErrorCode::HuggingFaceSetupNotFound
        }
        HuggingFaceSetupError::HuggingFaceApiKeyRequired => {
            NativeCommandErrorCode::HuggingFaceApiKeyRequired
        }
        HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized => {
            NativeCommandErrorCode::HuggingFaceApiKeyUnauthorized
        }
        HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions => {
            NativeCommandErrorCode::HuggingFaceApiKeyInsufficientPermissions
        }
        HuggingFaceSetupError::StoredHuggingFaceApiKeyInvalid => {
            NativeCommandErrorCode::StoredHuggingFaceApiKeyInvalid
        }
        HuggingFaceSetupError::HuggingFaceApiUnavailable => {
            NativeCommandErrorCode::HuggingFaceApiUnavailable
        }
        HuggingFaceSetupError::HuggingFaceRateLimited => {
            NativeCommandErrorCode::HuggingFaceRateLimited
        }
        HuggingFaceSetupError::HuggingFaceIdentityResponseInvalid => {
            NativeCommandErrorCode::HuggingFaceIdentityResponseInvalid
        }
        HuggingFaceSetupError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
    }
}

fn hugging_face_setup_error_retryable(error: &HuggingFaceSetupError) -> bool {
    matches!(
        error,
        HuggingFaceSetupError::HuggingFaceApiUnavailable
            | HuggingFaceSetupError::HuggingFaceRateLimited
            | HuggingFaceSetupError::SecureKeyringUnavailable
    )
}

fn hugging_face_setup_error_message(error: &HuggingFaceSetupError) -> &'static str {
    match error {
        HuggingFaceSetupError::HuggingFaceSetupNotFound => {
            "Hugging Face API key setup was not found."
        }
        HuggingFaceSetupError::HuggingFaceApiKeyRequired => "Hugging Face API key is required.",
        HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized => {
            "Hugging Face API key is not authorized."
        }
        HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions => {
            "Hugging Face API key has insufficient permissions."
        }
        HuggingFaceSetupError::StoredHuggingFaceApiKeyInvalid => {
            "Stored Hugging Face API key is invalid."
        }
        HuggingFaceSetupError::HuggingFaceApiUnavailable => "Hugging Face API is unavailable.",
        HuggingFaceSetupError::HuggingFaceRateLimited => "Hugging Face API is rate limited.",
        HuggingFaceSetupError::HuggingFaceIdentityResponseInvalid => {
            "Hugging Face identity response is invalid."
        }
        HuggingFaceSetupError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
    }
}

fn hugging_face_setup_error_field(error: &HuggingFaceSetupError) -> Option<&'static str> {
    match error {
        HuggingFaceSetupError::HuggingFaceApiKeyRequired
        | HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized
        | HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions => {
            Some("hugging_face_api_key")
        }
        _ => None,
    }
}

fn hugging_face_setup_error_recovery_action(error: &HuggingFaceSetupError) -> Option<&'static str> {
    match error {
        HuggingFaceSetupError::HuggingFaceSetupNotFound => Some("refresh_hugging_face_setup"),
        HuggingFaceSetupError::HuggingFaceApiKeyRequired
        | HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized
        | HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions => {
            Some("enter_hugging_face_api_key")
        }
        HuggingFaceSetupError::StoredHuggingFaceApiKeyInvalid
        | HuggingFaceSetupError::HuggingFaceIdentityResponseInvalid => {
            Some("recover_hugging_face_setup")
        }
        HuggingFaceSetupError::HuggingFaceApiUnavailable
        | HuggingFaceSetupError::HuggingFaceRateLimited
        | HuggingFaceSetupError::SecureKeyringUnavailable => Some("retry"),
    }
}

fn provider_setup_error_code(error: &ProviderSetupError) -> NativeCommandErrorCode {
    match error {
        ProviderSetupError::ProviderSetupNotFound => NativeCommandErrorCode::ProviderSetupNotFound,
        ProviderSetupError::ProviderSetupAlreadyExists => {
            NativeCommandErrorCode::ProviderSetupAlreadyExists
        }
        ProviderSetupError::ProviderApiKeyRequired => {
            NativeCommandErrorCode::ProviderApiKeyRequired
        }
        ProviderSetupError::ProviderApiKeyUnauthorized => {
            NativeCommandErrorCode::ProviderApiKeyUnauthorized
        }
        ProviderSetupError::StoredProviderApiKeyInvalid => {
            NativeCommandErrorCode::StoredProviderApiKeyInvalid
        }
        ProviderSetupError::ProviderApiUnavailable => {
            NativeCommandErrorCode::ProviderApiUnavailable
        }
        ProviderSetupError::ProviderIdentityResponseInvalid => {
            NativeCommandErrorCode::ProviderIdentityResponseInvalid
        }
        ProviderSetupError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
        ProviderSetupError::ProviderSetupRecoveryRequired => {
            NativeCommandErrorCode::ProviderSetupRecoveryRequired
        }
    }
}

fn provider_setup_error_retryable(error: &ProviderSetupError) -> bool {
    matches!(
        error,
        ProviderSetupError::ProviderApiUnavailable | ProviderSetupError::SecureKeyringUnavailable
    )
}

fn provider_setup_error_message(error: &ProviderSetupError) -> &'static str {
    match error {
        ProviderSetupError::ProviderSetupNotFound => "GPU cloud provider setup was not found.",
        ProviderSetupError::ProviderSetupAlreadyExists => {
            "GPU cloud provider setup already exists."
        }
        ProviderSetupError::ProviderApiKeyRequired => "Provider API key is required.",
        ProviderSetupError::ProviderApiKeyUnauthorized => "Provider API key is not authorized.",
        ProviderSetupError::StoredProviderApiKeyInvalid => "Stored provider API key is invalid.",
        ProviderSetupError::ProviderApiUnavailable => "Provider API is unavailable.",
        ProviderSetupError::ProviderIdentityResponseInvalid => {
            "Provider identity response is invalid."
        }
        ProviderSetupError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        ProviderSetupError::ProviderSetupRecoveryRequired => {
            "GPU cloud provider setup requires local recovery."
        }
    }
}

fn provider_setup_error_field(error: &ProviderSetupError) -> Option<&'static str> {
    match error {
        ProviderSetupError::ProviderApiKeyRequired
        | ProviderSetupError::ProviderApiKeyUnauthorized => Some("provider_api_key"),
        _ => None,
    }
}

fn provider_setup_error_recovery_action(error: &ProviderSetupError) -> Option<&'static str> {
    match error {
        ProviderSetupError::ProviderSetupNotFound => Some("refresh_provider_setup"),
        ProviderSetupError::ProviderSetupAlreadyExists => Some("refresh_provider_setup"),
        ProviderSetupError::ProviderApiKeyRequired
        | ProviderSetupError::ProviderApiKeyUnauthorized => Some("enter_provider_api_key"),
        ProviderSetupError::StoredProviderApiKeyInvalid
        | ProviderSetupError::ProviderIdentityResponseInvalid
        | ProviderSetupError::ProviderSetupRecoveryRequired => Some("recover_provider_setup"),
        ProviderSetupError::ProviderApiUnavailable
        | ProviderSetupError::SecureKeyringUnavailable => Some("retry"),
    }
}

fn error_code(error: &WorkspaceSetupError) -> NativeCommandErrorCode {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => {
            NativeCommandErrorCode::ProviderSetupIncomplete
        }
        WorkspaceSetupError::ProviderApiKeyUnauthorized => {
            NativeCommandErrorCode::ProviderApiKeyUnauthorized
        }
        WorkspaceSetupError::StoredProviderApiKeyInvalid => {
            NativeCommandErrorCode::StoredProviderApiKeyInvalid
        }
        WorkspaceSetupError::ProviderApiUnavailable => {
            NativeCommandErrorCode::ProviderApiUnavailable
        }
        WorkspaceSetupError::ProviderRateLimited => NativeCommandErrorCode::ProviderRateLimited,
        WorkspaceSetupError::ProviderRequestRejected => {
            NativeCommandErrorCode::ProviderRequestRejected
        }
        WorkspaceSetupError::ProviderResponseInvalid => {
            NativeCommandErrorCode::ProviderResponseInvalid
        }
        WorkspaceSetupError::ProviderInventoryInvalid => {
            NativeCommandErrorCode::ProviderInventoryInvalid
        }
        WorkspaceSetupError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
        WorkspaceSetupError::WorkflowCatalogUnavailable => {
            NativeCommandErrorCode::WorkflowCatalogUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogUnavailable => {
            NativeCommandErrorCode::WorkspaceCatalogUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
            NativeCommandErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
            NativeCommandErrorCode::WorkspaceCatalogMigrationFailed
        }
        WorkspaceSetupError::WorkspaceCatalogQueryFailed => {
            NativeCommandErrorCode::WorkspaceCatalogQueryFailed
        }
        WorkspaceSetupError::WorkspaceCatalogCorrupt => {
            NativeCommandErrorCode::WorkspaceCatalogCorrupt
        }
        WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
            NativeCommandErrorCode::WorkspaceCatalogSchemaMismatch
        }
        WorkspaceSetupError::PlacementProviderMismatch => {
            NativeCommandErrorCode::PlacementProviderMismatch
        }
        WorkspaceSetupError::PlacementDatacenterRequired => {
            NativeCommandErrorCode::PlacementDatacenterRequired
        }
        WorkspaceSetupError::PlacementGpuRequired => NativeCommandErrorCode::PlacementGpuRequired,
        WorkspaceSetupError::WorkflowPresetStale => NativeCommandErrorCode::WorkflowPresetStale,
        WorkspaceSetupError::StorageSizeBelowPresetMinimum => {
            NativeCommandErrorCode::StorageSizeBelowPresetMinimum
        }
        WorkspaceSetupError::EndpointKeepAliveOutOfRange => {
            NativeCommandErrorCode::EndpointKeepAliveOutOfRange
        }
        WorkspaceSetupError::WorkspaceAlreadyExists => {
            NativeCommandErrorCode::WorkspaceAlreadyExists
        }
        WorkspaceSetupError::InvalidWorkspaceId => NativeCommandErrorCode::InvalidWorkspaceId,
        WorkspaceSetupError::WorkspaceNameRequired => NativeCommandErrorCode::WorkspaceNameRequired,
        WorkspaceSetupError::InvalidWorkspaceMetadata => {
            NativeCommandErrorCode::InvalidWorkspaceMetadata
        }
    }
}

fn error_retryable(error: &WorkspaceSetupError) -> bool {
    matches!(
        error,
        WorkspaceSetupError::ProviderApiUnavailable
            | WorkspaceSetupError::ProviderRateLimited
            | WorkspaceSetupError::SecureKeyringUnavailable
            | WorkspaceSetupError::WorkspaceCatalogUnavailable
            | WorkspaceSetupError::WorkspaceCatalogStorageUnavailable
            | WorkspaceSetupError::WorkspaceCatalogMigrationFailed
            | WorkspaceSetupError::WorkspaceCatalogQueryFailed
    )
}

fn error_message(error: &WorkspaceSetupError) -> &'static str {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => "GPU cloud provider setup is incomplete.",
        WorkspaceSetupError::ProviderApiKeyUnauthorized => "Provider API key is not authorized.",
        WorkspaceSetupError::StoredProviderApiKeyInvalid => "Stored provider API key is invalid.",
        WorkspaceSetupError::ProviderApiUnavailable => "Provider API is unavailable.",
        WorkspaceSetupError::ProviderRateLimited => "Provider API rate limit was reached.",
        WorkspaceSetupError::ProviderRequestRejected => "Provider request was rejected.",
        WorkspaceSetupError::ProviderResponseInvalid => "Provider response is invalid.",
        WorkspaceSetupError::ProviderInventoryInvalid => "Provider inventory is invalid.",
        WorkspaceSetupError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        WorkspaceSetupError::WorkflowCatalogUnavailable => "Workflow catalog is unavailable.",
        WorkspaceSetupError::WorkspaceCatalogUnavailable => "Workspace catalog is unavailable.",
        WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
            "Workspace catalog storage is unavailable."
        }
        WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
            "Workspace catalog migration failed."
        }
        WorkspaceSetupError::WorkspaceCatalogQueryFailed => "Workspace catalog query failed.",
        WorkspaceSetupError::WorkspaceCatalogCorrupt => "Workspace catalog data is corrupt.",
        WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
            "Workspace catalog row data is inconsistent."
        }
        WorkspaceSetupError::PlacementProviderMismatch => "Placement provider does not match.",
        WorkspaceSetupError::PlacementDatacenterRequired => "Placement datacenter is required.",
        WorkspaceSetupError::PlacementGpuRequired => "Placement GPU is required.",
        WorkspaceSetupError::WorkflowPresetStale => "Workflow preset selection is stale.",
        WorkspaceSetupError::StorageSizeBelowPresetMinimum => {
            "Storage size is below the selected preset minimum."
        }
        WorkspaceSetupError::EndpointKeepAliveOutOfRange => {
            "Endpoint keep-alive is outside the provider-supported range."
        }
        WorkspaceSetupError::WorkspaceAlreadyExists => "Workspace already exists.",
        WorkspaceSetupError::InvalidWorkspaceId => "Workspace ID must be a valid UUID.",
        WorkspaceSetupError::WorkspaceNameRequired => "Workspace name is required.",
        WorkspaceSetupError::InvalidWorkspaceMetadata => "Workspace metadata is invalid.",
    }
}

fn error_field(error: &WorkspaceSetupError) -> Option<&'static str> {
    match error {
        WorkspaceSetupError::ProviderApiKeyUnauthorized
        | WorkspaceSetupError::StoredProviderApiKeyInvalid => Some("provider_api_key"),
        WorkspaceSetupError::InvalidWorkspaceId => Some("workspace_id"),
        WorkspaceSetupError::WorkspaceNameRequired => Some("name"),
        WorkspaceSetupError::PlacementDatacenterRequired => Some("selected_datacenter_id"),
        WorkspaceSetupError::PlacementGpuRequired => Some("selected_gpu_id"),
        WorkspaceSetupError::WorkflowPresetStale => Some("selected_workflow_preset"),
        WorkspaceSetupError::StorageSizeBelowPresetMinimum => {
            Some("persistent_storage_volume_size_bytes")
        }
        WorkspaceSetupError::EndpointKeepAliveOutOfRange => Some("endpoint_keep_alive_seconds"),
        _ => None,
    }
}

fn error_recovery_action(error: &WorkspaceSetupError) -> Option<&'static str> {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => Some("setup_provider"),
        WorkspaceSetupError::ProviderApiKeyUnauthorized
        | WorkspaceSetupError::StoredProviderApiKeyInvalid => Some("recover_provider_setup"),
        WorkspaceSetupError::ProviderApiUnavailable
        | WorkspaceSetupError::ProviderRateLimited
        | WorkspaceSetupError::SecureKeyringUnavailable
        | WorkspaceSetupError::WorkspaceCatalogUnavailable
        | WorkspaceSetupError::WorkspaceCatalogStorageUnavailable
        | WorkspaceSetupError::WorkspaceCatalogMigrationFailed
        | WorkspaceSetupError::WorkspaceCatalogQueryFailed => Some("retry"),
        WorkspaceSetupError::WorkflowCatalogUnavailable
        | WorkspaceSetupError::WorkflowPresetStale => Some("reload_workflow_presets"),
        WorkspaceSetupError::ProviderResponseInvalid
        | WorkspaceSetupError::ProviderRequestRejected
        | WorkspaceSetupError::ProviderInventoryInvalid => Some("retry_provider_inventory"),
        WorkspaceSetupError::WorkspaceCatalogCorrupt
        | WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => Some("recover_workspace_catalog"),
        WorkspaceSetupError::PlacementProviderMismatch
        | WorkspaceSetupError::PlacementDatacenterRequired
        | WorkspaceSetupError::PlacementGpuRequired
        | WorkspaceSetupError::StorageSizeBelowPresetMinimum
        | WorkspaceSetupError::EndpointKeepAliveOutOfRange => Some("reselect_placement"),
        WorkspaceSetupError::WorkspaceAlreadyExists => Some("refresh_workspace_catalog"),
        WorkspaceSetupError::InvalidWorkspaceId
        | WorkspaceSetupError::WorkspaceNameRequired
        | WorkspaceSetupError::InvalidWorkspaceMetadata => Some("change_request"),
    }
}

fn provisioning_error_code(error: &WorkspaceProvisioningError) -> NativeCommandErrorCode {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => NativeCommandErrorCode::WorkspaceNotFound,
        WorkspaceProvisioningError::InvalidWorkspaceLifecycle => {
            NativeCommandErrorCode::InvalidWorkspaceLifecycle
        }
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable => {
            NativeCommandErrorCode::WorkspaceCatalogUnavailable
        }
        WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable => {
            NativeCommandErrorCode::WorkspaceCatalogStorageUnavailable
        }
        WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed => {
            NativeCommandErrorCode::WorkspaceCatalogMigrationFailed
        }
        WorkspaceProvisioningError::WorkspaceCatalogQueryFailed => {
            NativeCommandErrorCode::WorkspaceCatalogQueryFailed
        }
        WorkspaceProvisioningError::WorkspaceCatalogCorrupt => {
            NativeCommandErrorCode::WorkspaceCatalogCorrupt
        }
        WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch => {
            NativeCommandErrorCode::WorkspaceCatalogSchemaMismatch
        }
        WorkspaceProvisioningError::ProviderSetupIncomplete => {
            NativeCommandErrorCode::ProviderSetupIncomplete
        }
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => {
            NativeCommandErrorCode::ProviderApiKeyUnauthorized
        }
        WorkspaceProvisioningError::ProviderApiUnavailable => {
            NativeCommandErrorCode::ProviderApiUnavailable
        }
        WorkspaceProvisioningError::ProviderRateLimited => {
            NativeCommandErrorCode::ProviderRateLimited
        }
        WorkspaceProvisioningError::ProviderRequestRejected => {
            NativeCommandErrorCode::ProviderRequestRejected
        }
        WorkspaceProvisioningError::ProviderResponseInvalid => {
            NativeCommandErrorCode::ProviderResponseInvalid
        }
        WorkspaceProvisioningError::ProviderResourceNotFound => {
            NativeCommandErrorCode::ProviderResourceNotFound
        }
        WorkspaceProvisioningError::ProviderOrphanedResources => {
            NativeCommandErrorCode::ProviderOrphanedResources
        }
        WorkspaceProvisioningError::ProviderOperationConflict => {
            NativeCommandErrorCode::ProviderOperationConflict
        }
        WorkspaceProvisioningError::ProviderOperationIndeterminate => {
            NativeCommandErrorCode::ProviderOperationIndeterminate
        }
        WorkspaceProvisioningError::HuggingFaceApiKeySetupRequired => {
            NativeCommandErrorCode::HuggingFaceApiKeySetupRequired
        }
        WorkspaceProvisioningError::CleanupFailed => NativeCommandErrorCode::CleanupFailed,
        WorkspaceProvisioningError::SecureKeyringUnavailable => {
            NativeCommandErrorCode::SecureKeyringUnavailable
        }
        WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid => {
            NativeCommandErrorCode::ProvisionerWorkerTokenInvalid
        }
        WorkspaceProvisioningError::ProvisionerWorkerUnauthorized => {
            NativeCommandErrorCode::ProvisionerWorkerUnauthorized
        }
        WorkspaceProvisioningError::ProvisionerWorkerUnavailable => {
            NativeCommandErrorCode::ProvisionerWorkerUnavailable
        }
        WorkspaceProvisioningError::ProvisionerWorkerConflict => {
            NativeCommandErrorCode::ProvisionerWorkerConflict
        }
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid => {
            NativeCommandErrorCode::ProvisionerWorkerResponseInvalid
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed => {
            NativeCommandErrorCode::ProvisionerWorkerFailed
        }
    }
}

fn provisioning_error_retryable(error: &WorkspaceProvisioningError) -> bool {
    matches!(
        error,
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable
            | WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable
            | WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed
            | WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
            | WorkspaceProvisioningError::ProviderApiUnavailable
            | WorkspaceProvisioningError::ProviderRateLimited
            | WorkspaceProvisioningError::ProviderOperationConflict
            | WorkspaceProvisioningError::ProviderOperationIndeterminate
            | WorkspaceProvisioningError::SecureKeyringUnavailable
            | WorkspaceProvisioningError::ProvisionerWorkerUnavailable
            | WorkspaceProvisioningError::ProvisionerWorkerConflict
    )
}

fn provisioning_error_message(error: &WorkspaceProvisioningError) -> &'static str {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => "Workspace was not found.",
        WorkspaceProvisioningError::InvalidWorkspaceLifecycle => {
            "Workspace lifecycle does not allow this provisioning operation."
        }
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable => {
            "Workspace catalog is unavailable."
        }
        WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable => {
            "Workspace catalog storage is unavailable."
        }
        WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed => {
            "Workspace catalog migration failed."
        }
        WorkspaceProvisioningError::WorkspaceCatalogQueryFailed => {
            "Workspace catalog query failed."
        }
        WorkspaceProvisioningError::WorkspaceCatalogCorrupt => "Workspace catalog data is corrupt.",
        WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch => {
            "Workspace catalog row data is inconsistent."
        }
        WorkspaceProvisioningError::ProviderSetupIncomplete => {
            "GPU cloud provider setup is incomplete."
        }
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => {
            "Provider API key is not authorized."
        }
        WorkspaceProvisioningError::ProviderApiUnavailable => "Provider API is unavailable.",
        WorkspaceProvisioningError::ProviderRateLimited => "Provider API rate limit was reached.",
        WorkspaceProvisioningError::ProviderRequestRejected => "Provider request was rejected.",
        WorkspaceProvisioningError::ProviderResponseInvalid => "Provider response is invalid.",
        WorkspaceProvisioningError::ProviderResourceNotFound => "Provider resource was not found.",
        WorkspaceProvisioningError::ProviderOrphanedResources => {
            "Provider has workspace-owned resources that cannot be safely adopted."
        }
        WorkspaceProvisioningError::ProviderOperationConflict => {
            "Provider operation is currently in conflict."
        }
        WorkspaceProvisioningError::ProviderOperationIndeterminate => {
            "Provider operation result is indeterminate."
        }
        WorkspaceProvisioningError::HuggingFaceApiKeySetupRequired => {
            "Hugging Face API key setup is required."
        }
        WorkspaceProvisioningError::CleanupFailed => "Workspace resource cleanup failed.",
        WorkspaceProvisioningError::SecureKeyringUnavailable => "Secure keyring is unavailable.",
        WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid => {
            "Provisioner worker token is invalid."
        }
        WorkspaceProvisioningError::ProvisionerWorkerUnauthorized => {
            "Provisioner worker authorization failed."
        }
        WorkspaceProvisioningError::ProvisionerWorkerUnavailable => {
            "Provisioner worker is unavailable."
        }
        WorkspaceProvisioningError::ProvisionerWorkerConflict => {
            "Provisioner worker operation is currently in conflict."
        }
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid => {
            "Provisioner worker response is invalid."
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed => "Provisioner worker failed.",
    }
}

fn provisioning_error_field(error: &WorkspaceProvisioningError) -> Option<&'static str> {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => Some("workspace_id"),
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => Some("provider_api_key"),
        WorkspaceProvisioningError::HuggingFaceApiKeySetupRequired => Some("hugging_face_api_key"),
        _ => None,
    }
}

fn provisioning_error_recovery_action(error: &WorkspaceProvisioningError) -> Option<&'static str> {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => Some("refresh_workspace_catalog"),
        WorkspaceProvisioningError::InvalidWorkspaceLifecycle => Some("refresh_workspace"),
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable
        | WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable
        | WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed
        | WorkspaceProvisioningError::WorkspaceCatalogQueryFailed
        | WorkspaceProvisioningError::ProviderApiUnavailable
        | WorkspaceProvisioningError::ProviderRateLimited
        | WorkspaceProvisioningError::ProviderOperationConflict
        | WorkspaceProvisioningError::ProviderOperationIndeterminate
        | WorkspaceProvisioningError::CleanupFailed
        | WorkspaceProvisioningError::SecureKeyringUnavailable
        | WorkspaceProvisioningError::ProvisionerWorkerUnavailable
        | WorkspaceProvisioningError::ProvisionerWorkerConflict => Some("retry"),
        WorkspaceProvisioningError::WorkspaceCatalogCorrupt
        | WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch => {
            Some("recover_workspace_catalog")
        }
        WorkspaceProvisioningError::ProviderSetupIncomplete
        | WorkspaceProvisioningError::ProviderApiKeyUnauthorized => Some("recover_provider_setup"),
        WorkspaceProvisioningError::HuggingFaceApiKeySetupRequired => {
            Some("configure_hugging_face_setup")
        }
        WorkspaceProvisioningError::ProviderRequestRejected => Some("reselect_placement"),
        WorkspaceProvisioningError::ProviderResponseInvalid
        | WorkspaceProvisioningError::ProviderResourceNotFound
        | WorkspaceProvisioningError::ProviderOrphanedResources
        | WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid
        | WorkspaceProvisioningError::ProvisionerWorkerUnauthorized
        | WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid
        | WorkspaceProvisioningError::ProvisionerWorkerFailed => {
            Some("inspect_workspace_provisioning")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hugging_face_setup_errors_map_to_safe_command_errors() {
        for (error, code, retryable, field, recovery_action) in [
            (
                HuggingFaceSetupError::HuggingFaceApiKeyRequired,
                NativeCommandErrorCode::HuggingFaceApiKeyRequired,
                false,
                Some("hugging_face_api_key"),
                "enter_hugging_face_api_key",
            ),
            (
                HuggingFaceSetupError::HuggingFaceApiKeyUnauthorized,
                NativeCommandErrorCode::HuggingFaceApiKeyUnauthorized,
                false,
                Some("hugging_face_api_key"),
                "enter_hugging_face_api_key",
            ),
            (
                HuggingFaceSetupError::HuggingFaceApiKeyInsufficientPermissions,
                NativeCommandErrorCode::HuggingFaceApiKeyInsufficientPermissions,
                false,
                Some("hugging_face_api_key"),
                "enter_hugging_face_api_key",
            ),
            (
                HuggingFaceSetupError::HuggingFaceApiUnavailable,
                NativeCommandErrorCode::HuggingFaceApiUnavailable,
                true,
                None,
                "retry",
            ),
            (
                HuggingFaceSetupError::HuggingFaceIdentityResponseInvalid,
                NativeCommandErrorCode::HuggingFaceIdentityResponseInvalid,
                false,
                None,
                "recover_hugging_face_setup",
            ),
        ] {
            let command_error = NativeCommandError::from(error);

            assert_eq!(command_error.code, code);
            assert_eq!(command_error.retryable, retryable);
            assert_eq!(command_error.field.as_deref(), field);
            assert_eq!(
                command_error.recovery_action.as_deref(),
                Some(recovery_action)
            );
        }
    }

    #[test]
    fn provisioning_catalog_errors_map_to_granular_command_errors() {
        for (error, code, retryable, recovery_action) in [
            (
                WorkspaceProvisioningError::WorkspaceCatalogUnavailable,
                NativeCommandErrorCode::WorkspaceCatalogUnavailable,
                true,
                "retry",
            ),
            (
                WorkspaceProvisioningError::WorkspaceCatalogStorageUnavailable,
                NativeCommandErrorCode::WorkspaceCatalogStorageUnavailable,
                true,
                "retry",
            ),
            (
                WorkspaceProvisioningError::WorkspaceCatalogMigrationFailed,
                NativeCommandErrorCode::WorkspaceCatalogMigrationFailed,
                true,
                "retry",
            ),
            (
                WorkspaceProvisioningError::WorkspaceCatalogQueryFailed,
                NativeCommandErrorCode::WorkspaceCatalogQueryFailed,
                true,
                "retry",
            ),
            (
                WorkspaceProvisioningError::WorkspaceCatalogCorrupt,
                NativeCommandErrorCode::WorkspaceCatalogCorrupt,
                false,
                "recover_workspace_catalog",
            ),
            (
                WorkspaceProvisioningError::WorkspaceCatalogSchemaMismatch,
                NativeCommandErrorCode::WorkspaceCatalogSchemaMismatch,
                false,
                "recover_workspace_catalog",
            ),
        ] {
            let command_error = NativeCommandError::from(error);

            assert_eq!(command_error.code, code);
            assert_eq!(command_error.retryable, retryable);
            assert!(command_error.field.is_none());
            assert_eq!(
                command_error.recovery_action.as_deref(),
                Some(recovery_action)
            );
        }
    }

    #[test]
    fn provisioning_command_errors_map_stable_recovery_metadata() {
        for (error, code, retryable, field, recovery_action) in [
            (
                WorkspaceProvisioningError::SecureKeyringUnavailable,
                NativeCommandErrorCode::SecureKeyringUnavailable,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProviderApiUnavailable,
                NativeCommandErrorCode::ProviderApiUnavailable,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProviderRateLimited,
                NativeCommandErrorCode::ProviderRateLimited,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProviderOperationConflict,
                NativeCommandErrorCode::ProviderOperationConflict,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProvisionerWorkerUnavailable,
                NativeCommandErrorCode::ProvisionerWorkerUnavailable,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProvisionerWorkerConflict,
                NativeCommandErrorCode::ProvisionerWorkerConflict,
                true,
                None,
                "retry",
            ),
            (
                WorkspaceProvisioningError::ProviderRequestRejected,
                NativeCommandErrorCode::ProviderRequestRejected,
                false,
                None,
                "reselect_placement",
            ),
            (
                WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid,
                NativeCommandErrorCode::ProvisionerWorkerTokenInvalid,
                false,
                None,
                "inspect_workspace_provisioning",
            ),
            (
                WorkspaceProvisioningError::ProviderApiKeyUnauthorized,
                NativeCommandErrorCode::ProviderApiKeyUnauthorized,
                false,
                Some("provider_api_key"),
                "recover_provider_setup",
            ),
        ] {
            let command_error = NativeCommandError::from(error);

            assert_eq!(command_error.code, code);
            assert_eq!(command_error.retryable, retryable);
            assert_eq!(command_error.field.as_deref(), field);
            assert_eq!(
                command_error.recovery_action.as_deref(),
                Some(recovery_action)
            );
        }
    }
}
