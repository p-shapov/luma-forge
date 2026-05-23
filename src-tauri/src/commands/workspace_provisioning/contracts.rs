use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::workspace as domain_workspace, workspace_provisioning::WorkspaceProvisioningResult,
};

#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningStatus)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningStatus {
        Idle,
        Running,
        Cancelling,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningPhase)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningPhase {
        NotStarted,
        CreatingVolume,
        StartingProvisioningPod,
        PreparingEnvironment,
        CreatingEndpoint,
        ValidatingReadiness,
        CleaningUp,
        Completed,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningFailureCode)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningFailureCode {
        ProviderResourceFailed,
        ProviderResourceTerminated,
        ProviderResourceUnknown,
        ProviderResourceMissing,
        ProviderOrphanedResources,
        ProviderSetupIncomplete,
        ProviderApiKeyUnauthorized,
        ProviderApiUnavailable,
        ProviderRateLimited,
        ProviderRequestRejected,
        ProviderResponseInvalid,
        ProviderOperationConflict,
        ProviderOperationIndeterminate,
        SecureKeyringUnavailable,
        ProvisionerWorkerTokenMissing,
        ProvisionerWorkerTokenInvalid,
        ProvisionerWorkerUnauthorized,
        ProvisionerWorkerUnavailable,
        ProvisionerWorkerConflict,
        ProvisionerWorkerResponseInvalid,
        ProvisionerWorkerFailed,
        ProvisionerWorkerAssetDownloadFailed,
        ProvisionerWorkerAssetAuthRequired,
        ProvisionerWorkerPathValidationFailed,
        ProvisionerWorkerStepTimeout,
        ProvisionerWorkerUnexpectedError,
        ReadinessValidationFailed,
        CancellationCleanupFailed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningFailureSource)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningFailureSource {
        Native,
        Provider,
        ProviderResource,
        ProvisionerWorker,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningRecoveryAction)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceProvisioningRecoveryAction {
        Retry,
        RecoverProviderSetup,
        ReselectPlacement,
        InspectWorkspaceProvisioning,
        CleanupWorkspaceResources,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningFailure)]
    pub(super) struct WorkspaceProvisioningFailure {
        pub code: domain_workspace::WorkspaceProvisioningFailureCode,
        pub phase: domain_workspace::WorkspaceProvisioningPhase,
        pub source: domain_workspace::WorkspaceProvisioningFailureSource,
        pub recovery_action: domain_workspace::WorkspaceProvisioningRecoveryAction,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceProvisioningProgress)]
    pub(super) struct WorkspaceProvisioningProgress {
        pub status: domain_workspace::WorkspaceProvisioningStatus,
        pub phase: domain_workspace::WorkspaceProvisioningPhase,
        pub percent: Option<u8>,
        pub failure: Option<domain_workspace::WorkspaceProvisioningFailure>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WorkspaceProvisioningRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WorkspaceProvisioningResponse {
    pub workspace: domain_workspace::Workspace,
    pub progress: domain_workspace::WorkspaceProvisioningProgress,
}

impl From<WorkspaceProvisioningResult> for WorkspaceProvisioningResponse {
    fn from(result: WorkspaceProvisioningResult) -> Self {
        Self {
            workspace: result.workspace,
            progress: result.progress,
        }
    }
}
