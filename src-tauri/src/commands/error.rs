use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    provider_setup::ProviderSetupError, workspace_provisioning::WorkspaceProvisioningError,
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
    ProviderOperationConflict,
    ProviderOperationIndeterminate,
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
            Self::ProviderOperationConflict => "provider_operation_conflict",
            Self::ProviderOperationIndeterminate => "provider_operation_indeterminate",
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
    pub reason: Option<String>,
    pub recovery_action: Option<String>,
}

impl From<ProviderSetupError> for NativeCommandError {
    fn from(error: ProviderSetupError) -> Self {
        Self {
            code: provider_setup_error_code(&error),
            message: provider_setup_error_message(&error).to_string(),
            retryable: provider_setup_error_retryable(&error),
            field: provider_setup_error_field(&error).map(str::to_string),
            reason: provider_setup_error_reason(&error).map(str::to_string),
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
            reason: error_reason(&error).map(str::to_string),
            recovery_action: error_recovery_action(&error).map(str::to_string),
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
            reason: provisioning_error_reason(&error).map(str::to_string),
            recovery_action: provisioning_error_recovery_action(&error).map(str::to_string),
        }
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

fn provider_setup_error_reason(error: &ProviderSetupError) -> Option<&'static str> {
    match error {
        ProviderSetupError::ProviderSetupNotFound => Some("setup_not_found"),
        ProviderSetupError::ProviderSetupAlreadyExists => Some("setup_already_exists"),
        ProviderSetupError::ProviderApiKeyRequired => Some("missing_required_value"),
        ProviderSetupError::ProviderApiKeyUnauthorized => Some("provider_rejected_key"),
        ProviderSetupError::StoredProviderApiKeyInvalid => Some("stored_secret_invalid"),
        ProviderSetupError::ProviderApiUnavailable => Some("provider_unavailable"),
        ProviderSetupError::ProviderIdentityResponseInvalid => {
            Some("provider_identity_response_invalid")
        }
        ProviderSetupError::SecureKeyringUnavailable => Some("secure_keyring_unavailable"),
        ProviderSetupError::ProviderSetupRecoveryRequired => Some("local_recovery_required"),
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

fn error_reason(error: &WorkspaceSetupError) -> Option<&'static str> {
    match error {
        WorkspaceSetupError::ProviderSetupIncomplete => Some("setup_incomplete"),
        WorkspaceSetupError::ProviderApiKeyUnauthorized => Some("provider_rejected_key"),
        WorkspaceSetupError::StoredProviderApiKeyInvalid => Some("stored_secret_invalid"),
        WorkspaceSetupError::ProviderApiUnavailable => Some("provider_unavailable"),
        WorkspaceSetupError::ProviderRateLimited => Some("provider_rate_limited"),
        WorkspaceSetupError::ProviderRequestRejected => Some("provider_request_rejected"),
        WorkspaceSetupError::ProviderResponseInvalid => Some("provider_response_invalid"),
        WorkspaceSetupError::ProviderInventoryInvalid => Some("provider_inventory_invalid"),
        WorkspaceSetupError::SecureKeyringUnavailable => Some("secure_keyring_unavailable"),
        WorkspaceSetupError::WorkflowCatalogUnavailable => Some("workflow_catalog_unavailable"),
        WorkspaceSetupError::WorkspaceCatalogUnavailable => Some("workspace_catalog_unavailable"),
        WorkspaceSetupError::WorkspaceCatalogStorageUnavailable => {
            Some("workspace_catalog_storage_unavailable")
        }
        WorkspaceSetupError::WorkspaceCatalogMigrationFailed => {
            Some("workspace_catalog_migration_failed")
        }
        WorkspaceSetupError::WorkspaceCatalogQueryFailed => Some("workspace_catalog_query_failed"),
        WorkspaceSetupError::WorkspaceCatalogCorrupt => Some("workspace_catalog_corrupt"),
        WorkspaceSetupError::WorkspaceCatalogSchemaMismatch => {
            Some("workspace_catalog_schema_mismatch")
        }
        WorkspaceSetupError::PlacementProviderMismatch => Some("placement_provider_mismatch"),
        WorkspaceSetupError::PlacementDatacenterRequired => Some("missing_required_value"),
        WorkspaceSetupError::PlacementGpuRequired => Some("missing_required_value"),
        WorkspaceSetupError::WorkflowPresetStale => Some("stale_catalog_object"),
        WorkspaceSetupError::StorageSizeBelowPresetMinimum => Some("below_minimum"),
        WorkspaceSetupError::EndpointKeepAliveOutOfRange => Some("outside_allowed_range"),
        WorkspaceSetupError::WorkspaceAlreadyExists => Some("workspace_already_exists"),
        WorkspaceSetupError::InvalidWorkspaceId => Some("invalid_uuid"),
        WorkspaceSetupError::WorkspaceNameRequired => Some("missing_required_value"),
        WorkspaceSetupError::InvalidWorkspaceMetadata => Some("invalid_workspace_metadata"),
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
        WorkspaceProvisioningError::ProviderOperationConflict => {
            NativeCommandErrorCode::ProviderOperationConflict
        }
        WorkspaceProvisioningError::ProviderOperationIndeterminate => {
            NativeCommandErrorCode::ProviderOperationIndeterminate
        }
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
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid { .. } => {
            NativeCommandErrorCode::ProvisionerWorkerResponseInvalid
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed { .. } => {
            NativeCommandErrorCode::ProvisionerWorkerFailed
        }
    }
}

fn provisioning_error_retryable(error: &WorkspaceProvisioningError) -> bool {
    matches!(
        error,
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable
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
        WorkspaceProvisioningError::ProviderOperationConflict => {
            "Provider operation is currently in conflict."
        }
        WorkspaceProvisioningError::ProviderOperationIndeterminate => {
            "Provider operation result is indeterminate."
        }
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
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid { .. } => {
            "Provisioner worker response is invalid."
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed { .. } => "Provisioner worker failed.",
    }
}

fn provisioning_error_field(error: &WorkspaceProvisioningError) -> Option<&'static str> {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => Some("workspace_id"),
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => Some("provider_api_key"),
        _ => None,
    }
}

fn provisioning_error_reason(error: &WorkspaceProvisioningError) -> Option<&'static str> {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => Some("workspace_not_found"),
        WorkspaceProvisioningError::InvalidWorkspaceLifecycle => {
            Some("invalid_workspace_lifecycle")
        }
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable => {
            Some("workspace_catalog_unavailable")
        }
        WorkspaceProvisioningError::ProviderSetupIncomplete => Some("setup_incomplete"),
        WorkspaceProvisioningError::ProviderApiKeyUnauthorized => Some("provider_rejected_key"),
        WorkspaceProvisioningError::ProviderApiUnavailable => Some("provider_unavailable"),
        WorkspaceProvisioningError::ProviderRateLimited => Some("provider_rate_limited"),
        WorkspaceProvisioningError::ProviderRequestRejected => Some("provider_request_rejected"),
        WorkspaceProvisioningError::ProviderResponseInvalid => Some("provider_response_invalid"),
        WorkspaceProvisioningError::ProviderResourceNotFound => Some("provider_resource_not_found"),
        WorkspaceProvisioningError::ProviderOperationConflict => {
            Some("provider_operation_conflict")
        }
        WorkspaceProvisioningError::ProviderOperationIndeterminate => {
            Some("provider_operation_indeterminate")
        }
        WorkspaceProvisioningError::SecureKeyringUnavailable => Some("secure_keyring_unavailable"),
        WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid => Some("stored_secret_invalid"),
        WorkspaceProvisioningError::ProvisionerWorkerUnauthorized => Some("worker_unauthorized"),
        WorkspaceProvisioningError::ProvisionerWorkerUnavailable => Some("worker_unavailable"),
        WorkspaceProvisioningError::ProvisionerWorkerConflict => Some("worker_conflict"),
        WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid { .. } => {
            Some("worker_response_invalid")
        }
        WorkspaceProvisioningError::ProvisionerWorkerFailed { .. } => Some("worker_failed"),
    }
}

fn provisioning_error_recovery_action(error: &WorkspaceProvisioningError) -> Option<&'static str> {
    match error {
        WorkspaceProvisioningError::WorkspaceNotFound => Some("refresh_workspace_catalog"),
        WorkspaceProvisioningError::InvalidWorkspaceLifecycle => Some("refresh_workspace"),
        WorkspaceProvisioningError::WorkspaceCatalogUnavailable
        | WorkspaceProvisioningError::ProviderApiUnavailable
        | WorkspaceProvisioningError::ProviderRateLimited
        | WorkspaceProvisioningError::ProviderOperationConflict
        | WorkspaceProvisioningError::ProviderOperationIndeterminate
        | WorkspaceProvisioningError::SecureKeyringUnavailable
        | WorkspaceProvisioningError::ProvisionerWorkerUnavailable
        | WorkspaceProvisioningError::ProvisionerWorkerConflict => Some("retry"),
        WorkspaceProvisioningError::ProviderSetupIncomplete
        | WorkspaceProvisioningError::ProviderApiKeyUnauthorized => Some("recover_provider_setup"),
        WorkspaceProvisioningError::ProviderRequestRejected => Some("reselect_placement"),
        WorkspaceProvisioningError::ProviderResponseInvalid
        | WorkspaceProvisioningError::ProviderResourceNotFound
        | WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid
        | WorkspaceProvisioningError::ProvisionerWorkerUnauthorized
        | WorkspaceProvisioningError::ProvisionerWorkerResponseInvalid { .. }
        | WorkspaceProvisioningError::ProvisionerWorkerFailed { .. } => {
            Some("inspect_workspace_provisioning")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_setup_error_mapping_is_ui_safe() {
        let error = NativeCommandError::from(ProviderSetupError::ProviderApiKeyUnauthorized);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderApiKeyUnauthorized
        ));
        assert_eq!(error.message, "Provider API key is not authorized.");
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }

    #[test]
    fn provider_setup_recovery_required_mapping_is_ui_safe_and_not_retryable() {
        let error = NativeCommandError::from(ProviderSetupError::ProviderSetupRecoveryRequired);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderSetupRecoveryRequired
        ));
        assert_eq!(
            error.message,
            "GPU cloud provider setup requires local recovery."
        );
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }

    #[test]
    fn error_mapping_is_ui_safe() {
        let error = NativeCommandError::from(WorkspaceSetupError::ProviderApiUnavailable);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderApiUnavailable
        ));
        assert_eq!(error.message, "Provider API is unavailable.");
        assert!(!error.message.contains("rp_"));
        assert!(error.retryable);
    }

    #[test]
    fn workspace_invalid_provider_key_mapping_is_not_retryable() {
        let error = NativeCommandError::from(WorkspaceSetupError::ProviderApiKeyUnauthorized);

        assert!(matches!(
            error.code,
            NativeCommandErrorCode::ProviderApiKeyUnauthorized
        ));
        assert_eq!(error.message, "Provider API key is not authorized.");
        assert!(!error.message.contains("rp_"));
        assert!(!error.retryable);
    }

    #[test]
    fn workspace_provisioning_error_mapping_is_ui_safe() {
        let error = NativeCommandError::from(WorkspaceProvisioningError::ProvisionerWorkerFailed {
            diagnostic: Some("secret-looking diagnostic".to_string()),
        });

        assert_eq!(error.code, NativeCommandErrorCode::ProvisionerWorkerFailed);
        assert_eq!(error.message, "Provisioner worker failed.");
        assert_eq!(error.reason.as_deref(), Some("worker_failed"));
        assert_eq!(
            error.recovery_action.as_deref(),
            Some("inspect_workspace_provisioning")
        );
        assert!(!error.message.contains("secret-looking"));
        assert!(!error.retryable);
    }

    #[test]
    fn workspace_setup_provider_request_rejection_maps_to_non_retryable_recovery() {
        let error = NativeCommandError::from(WorkspaceSetupError::ProviderRequestRejected);

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRequestRejected);
        assert_eq!(error.reason.as_deref(), Some("provider_request_rejected"));
        assert_eq!(
            error.recovery_action.as_deref(),
            Some("retry_provider_inventory")
        );
        assert!(!error.retryable);
    }

    #[test]
    fn workspace_setup_provider_rate_limit_maps_to_retryable_recovery() {
        let error = NativeCommandError::from(WorkspaceSetupError::ProviderRateLimited);

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRateLimited);
        assert_eq!(error.reason.as_deref(), Some("provider_rate_limited"));
        assert_eq!(error.recovery_action.as_deref(), Some("retry"));
        assert!(error.retryable);
    }

    #[test]
    fn provisioning_request_rejection_maps_to_non_retryable_recovery() {
        let error = NativeCommandError::from(WorkspaceProvisioningError::ProviderRequestRejected);

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRequestRejected);
        assert_eq!(error.field, None);
        assert_eq!(error.reason.as_deref(), Some("provider_request_rejected"));
        assert_eq!(error.recovery_action.as_deref(), Some("reselect_placement"));
        assert!(!error.retryable);
    }

    #[test]
    fn provisioning_rate_limit_maps_to_retryable_recovery() {
        let error = NativeCommandError::from(WorkspaceProvisioningError::ProviderRateLimited);

        assert_eq!(error.code, NativeCommandErrorCode::ProviderRateLimited);
        assert_eq!(error.field, None);
        assert_eq!(error.reason.as_deref(), Some("provider_rate_limited"));
        assert_eq!(error.recovery_action.as_deref(), Some("retry"));
        assert!(error.retryable);
    }

    #[test]
    fn maps_every_provider_setup_error_variant() {
        let cases = [
            (
                ProviderSetupError::ProviderSetupNotFound,
                NativeCommandErrorCode::ProviderSetupNotFound,
                None,
                Some("setup_not_found"),
                Some("refresh_provider_setup"),
                false,
            ),
            (
                ProviderSetupError::ProviderSetupAlreadyExists,
                NativeCommandErrorCode::ProviderSetupAlreadyExists,
                None,
                Some("setup_already_exists"),
                Some("refresh_provider_setup"),
                false,
            ),
            (
                ProviderSetupError::ProviderApiKeyRequired,
                NativeCommandErrorCode::ProviderApiKeyRequired,
                Some("provider_api_key"),
                Some("missing_required_value"),
                Some("enter_provider_api_key"),
                false,
            ),
            (
                ProviderSetupError::ProviderApiKeyUnauthorized,
                NativeCommandErrorCode::ProviderApiKeyUnauthorized,
                Some("provider_api_key"),
                Some("provider_rejected_key"),
                Some("enter_provider_api_key"),
                false,
            ),
            (
                ProviderSetupError::StoredProviderApiKeyInvalid,
                NativeCommandErrorCode::StoredProviderApiKeyInvalid,
                None,
                Some("stored_secret_invalid"),
                Some("recover_provider_setup"),
                false,
            ),
            (
                ProviderSetupError::ProviderApiUnavailable,
                NativeCommandErrorCode::ProviderApiUnavailable,
                None,
                Some("provider_unavailable"),
                Some("retry"),
                true,
            ),
            (
                ProviderSetupError::ProviderIdentityResponseInvalid,
                NativeCommandErrorCode::ProviderIdentityResponseInvalid,
                None,
                Some("provider_identity_response_invalid"),
                Some("recover_provider_setup"),
                false,
            ),
            (
                ProviderSetupError::SecureKeyringUnavailable,
                NativeCommandErrorCode::SecureKeyringUnavailable,
                None,
                Some("secure_keyring_unavailable"),
                Some("retry"),
                true,
            ),
            (
                ProviderSetupError::ProviderSetupRecoveryRequired,
                NativeCommandErrorCode::ProviderSetupRecoveryRequired,
                None,
                Some("local_recovery_required"),
                Some("recover_provider_setup"),
                false,
            ),
        ];

        for (source, code, field, reason, recovery_action, retryable) in cases {
            let error = NativeCommandError::from(source);

            assert_eq!(error.code, code);
            assert_eq!(error.field.as_deref(), field);
            assert_eq!(error.reason.as_deref(), reason);
            assert_eq!(error.recovery_action.as_deref(), recovery_action);
            assert_eq!(error.retryable, retryable);
        }
    }

    #[test]
    fn maps_every_workspace_setup_error_variant() {
        let cases = [
            (
                WorkspaceSetupError::ProviderSetupIncomplete,
                NativeCommandErrorCode::ProviderSetupIncomplete,
                None,
                Some("setup_incomplete"),
                Some("setup_provider"),
                false,
            ),
            (
                WorkspaceSetupError::ProviderApiKeyUnauthorized,
                NativeCommandErrorCode::ProviderApiKeyUnauthorized,
                Some("provider_api_key"),
                Some("provider_rejected_key"),
                Some("recover_provider_setup"),
                false,
            ),
            (
                WorkspaceSetupError::StoredProviderApiKeyInvalid,
                NativeCommandErrorCode::StoredProviderApiKeyInvalid,
                Some("provider_api_key"),
                Some("stored_secret_invalid"),
                Some("recover_provider_setup"),
                false,
            ),
            (
                WorkspaceSetupError::ProviderApiUnavailable,
                NativeCommandErrorCode::ProviderApiUnavailable,
                None,
                Some("provider_unavailable"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::ProviderRateLimited,
                NativeCommandErrorCode::ProviderRateLimited,
                None,
                Some("provider_rate_limited"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::ProviderRequestRejected,
                NativeCommandErrorCode::ProviderRequestRejected,
                None,
                Some("provider_request_rejected"),
                Some("retry_provider_inventory"),
                false,
            ),
            (
                WorkspaceSetupError::ProviderResponseInvalid,
                NativeCommandErrorCode::ProviderResponseInvalid,
                None,
                Some("provider_response_invalid"),
                Some("retry_provider_inventory"),
                false,
            ),
            (
                WorkspaceSetupError::ProviderInventoryInvalid,
                NativeCommandErrorCode::ProviderInventoryInvalid,
                None,
                Some("provider_inventory_invalid"),
                Some("retry_provider_inventory"),
                false,
            ),
            (
                WorkspaceSetupError::SecureKeyringUnavailable,
                NativeCommandErrorCode::SecureKeyringUnavailable,
                None,
                Some("secure_keyring_unavailable"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::WorkflowCatalogUnavailable,
                NativeCommandErrorCode::WorkflowCatalogUnavailable,
                None,
                Some("workflow_catalog_unavailable"),
                Some("reload_workflow_presets"),
                false,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogUnavailable,
                NativeCommandErrorCode::WorkspaceCatalogUnavailable,
                None,
                Some("workspace_catalog_unavailable"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogStorageUnavailable,
                NativeCommandErrorCode::WorkspaceCatalogStorageUnavailable,
                None,
                Some("workspace_catalog_storage_unavailable"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogMigrationFailed,
                NativeCommandErrorCode::WorkspaceCatalogMigrationFailed,
                None,
                Some("workspace_catalog_migration_failed"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogQueryFailed,
                NativeCommandErrorCode::WorkspaceCatalogQueryFailed,
                None,
                Some("workspace_catalog_query_failed"),
                Some("retry"),
                true,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogCorrupt,
                NativeCommandErrorCode::WorkspaceCatalogCorrupt,
                None,
                Some("workspace_catalog_corrupt"),
                Some("recover_workspace_catalog"),
                false,
            ),
            (
                WorkspaceSetupError::WorkspaceCatalogSchemaMismatch,
                NativeCommandErrorCode::WorkspaceCatalogSchemaMismatch,
                None,
                Some("workspace_catalog_schema_mismatch"),
                Some("recover_workspace_catalog"),
                false,
            ),
            (
                WorkspaceSetupError::PlacementProviderMismatch,
                NativeCommandErrorCode::PlacementProviderMismatch,
                None,
                Some("placement_provider_mismatch"),
                Some("reselect_placement"),
                false,
            ),
            (
                WorkspaceSetupError::PlacementDatacenterRequired,
                NativeCommandErrorCode::PlacementDatacenterRequired,
                Some("selected_datacenter_id"),
                Some("missing_required_value"),
                Some("reselect_placement"),
                false,
            ),
            (
                WorkspaceSetupError::PlacementGpuRequired,
                NativeCommandErrorCode::PlacementGpuRequired,
                Some("selected_gpu_id"),
                Some("missing_required_value"),
                Some("reselect_placement"),
                false,
            ),
            (
                WorkspaceSetupError::WorkflowPresetStale,
                NativeCommandErrorCode::WorkflowPresetStale,
                Some("selected_workflow_preset"),
                Some("stale_catalog_object"),
                Some("reload_workflow_presets"),
                false,
            ),
            (
                WorkspaceSetupError::StorageSizeBelowPresetMinimum,
                NativeCommandErrorCode::StorageSizeBelowPresetMinimum,
                Some("persistent_storage_volume_size_bytes"),
                Some("below_minimum"),
                Some("reselect_placement"),
                false,
            ),
            (
                WorkspaceSetupError::EndpointKeepAliveOutOfRange,
                NativeCommandErrorCode::EndpointKeepAliveOutOfRange,
                Some("endpoint_keep_alive_seconds"),
                Some("outside_allowed_range"),
                Some("reselect_placement"),
                false,
            ),
            (
                WorkspaceSetupError::WorkspaceAlreadyExists,
                NativeCommandErrorCode::WorkspaceAlreadyExists,
                None,
                Some("workspace_already_exists"),
                Some("refresh_workspace_catalog"),
                false,
            ),
            (
                WorkspaceSetupError::InvalidWorkspaceId,
                NativeCommandErrorCode::InvalidWorkspaceId,
                Some("workspace_id"),
                Some("invalid_uuid"),
                Some("change_request"),
                false,
            ),
            (
                WorkspaceSetupError::WorkspaceNameRequired,
                NativeCommandErrorCode::WorkspaceNameRequired,
                Some("name"),
                Some("missing_required_value"),
                Some("change_request"),
                false,
            ),
            (
                WorkspaceSetupError::InvalidWorkspaceMetadata,
                NativeCommandErrorCode::InvalidWorkspaceMetadata,
                None,
                Some("invalid_workspace_metadata"),
                Some("change_request"),
                false,
            ),
        ];

        for (source, code, field, reason, recovery_action, retryable) in cases {
            let error = NativeCommandError::from(source);

            assert_eq!(error.code, code);
            assert_eq!(error.field.as_deref(), field);
            assert_eq!(error.reason.as_deref(), reason);
            assert_eq!(error.recovery_action.as_deref(), recovery_action);
            assert_eq!(error.retryable, retryable);
        }
    }
}
