use serde::{Deserialize, Serialize};

use super::{placement::RemotePlacementPlan, workflow_preset::WorkflowPreset};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteVolumeSnapshot {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProvisionerSnapshot {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteEndpointSnapshot {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed { code: String, message: String },
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningPhase {
    CreatingRemoteVolume,
    StartingRemoteProvisioner,
    RunningRemoteProvisioner {
        status: RemoteProvisionerStatus,
    },
    CleaningUpRemoteProvisioner {
        terminal_status: RemoteProvisionerStatus,
    },
    CreatingRemoteEndpoint,
    ValidatingReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningStatus {
    NotStarted,
    InProgress {
        phase: RemoteProvisioningPhase,
    },
    Cancelling {
        phase: Option<RemoteProvisioningPhase>,
    },
    Completed,
    Failed {
        phase: Option<RemoteProvisioningPhase>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteProvisioningState {
    pub status: RemoteProvisioningStatus,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWorkspaceResources {
    pub remote_volume: Option<RemoteVolumeSnapshot>,
    pub remote_provisioner: Option<RemoteProvisionerSnapshot>,
    pub remote_endpoint: Option<RemoteEndpointSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteWorkspace {
    pub remote_placement: RemotePlacementPlan,
    pub remote_provisioning: RemoteProvisioningState,
    pub remote_resources: RemoteWorkspaceResources,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkspaceRuntime {
    Remote(RemoteWorkspace),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub workflow_preset: WorkflowPreset,
    pub runtime: WorkspaceRuntime,
}
