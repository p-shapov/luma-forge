use serde::{Deserialize, Serialize};

use super::{
    placement::RemotePlacementPlan, provider::ProviderApiError, workflow_preset::WorkflowPreset,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningError {
    Provider(ProviderApiError),
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
    CancellationCleanupFailed,
    InvalidProvisioningState { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeVolumeSnapshot {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeProvisionerSnapshot {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeEndpointSnapshot {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisionerStatus {
    Pending,
    Starting,
    Running,
    CleaningUp,
    Succeeded,
    Failed { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningPhase {
    CreatingRemoteVolume,
    StartingRemoteProvisioner,
    RunningRemoteProvisioner {
        status: ProvisionedRemoteComputeProvisionerStatus,
    },
    CreatingRemoteEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningStatus {
    NotStarted,
    InProgress {
        phase: ProvisionedRemoteComputeProvisioningPhase,
    },
    Cancelling {
        phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
    },
    Completed,
    Failed {
        phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
        error: ProvisionedRemoteComputeProvisioningError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeProvisioningState {
    pub status: ProvisionedRemoteComputeProvisioningStatus,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeResources {
    pub volume: Option<ProvisionedRemoteComputeVolumeSnapshot>,
    pub provisioner: Option<ProvisionedRemoteComputeProvisionerSnapshot>,
    pub endpoint: Option<ProvisionedRemoteComputeEndpointSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedRemoteComputeWorkspace {
    pub remote_placement: RemotePlacementPlan,
    pub provisioning: ProvisionedRemoteComputeProvisioningState,
    pub resources: ProvisionedRemoteComputeResources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkspaceRuntime {
    ProvisionedRemoteCompute(ProvisionedRemoteComputeWorkspace),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub workflow_preset: WorkflowPreset,
    pub runtime: WorkspaceRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}
