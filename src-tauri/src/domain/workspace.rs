use serde::{Deserialize, Serialize};

use super::{placement::PlacementPlan, workflow_preset::WorkflowPreset};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSnapshot {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerSnapshot {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointSnapshot {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed { code: String, message: String },
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSetupPhase {
    CreatingVolume,
    StartingProvisioner,
    RunningProvisioner { status: ProvisionerStatus },
    CleaningUpProvisioner,
    CreatingEndpoint,
    ValidatingReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSetupStatus {
    NotStarted,
    InProgress {
        phase: WorkspaceSetupPhase,
    },
    Cancelling {
        phase: Option<WorkspaceSetupPhase>,
    },
    Completed,
    Failed {
        phase: Option<WorkspaceSetupPhase>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSetupProgress {
    pub status: WorkspaceSetupStatus,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub is_ready: bool,
    pub placement: PlacementPlan,
    pub workflow_preset: WorkflowPreset,
    pub setup_progress: Option<WorkspaceSetupProgress>,
    pub volume: Option<VolumeSnapshot>,
    pub provisioner: Option<ProvisionerSnapshot>,
    pub endpoint: Option<EndpointSnapshot>,
}
