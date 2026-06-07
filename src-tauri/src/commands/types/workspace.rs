use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::workspace::{
    RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
    RemoteProvisioningError, RemoteProvisioningPhase, RemoteProvisioningState,
    RemoteProvisioningStatus, RemoteVolumeSnapshot, RemoteWorkspace, RemoteWorkspaceResources,
    Workspace, WorkspaceCatalog, WorkspaceRuntime,
};

use super::{catalog::WorkflowPresetResponse, placement::RemotePlacementPlanInput};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCatalogResponse {
    pub workspaces: Vec<WorkspaceResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: String,
    pub workflow_preset: WorkflowPresetResponse,
    pub runtime: WorkspaceRuntimeResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum WorkspaceRuntimeResponse {
    Remote(RemoteWorkspaceResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceResponse {
    pub remote_placement: RemotePlacementPlanInput,
    pub remote_provisioning: RemoteProvisioningStateResponse,
    pub remote_resources: RemoteWorkspaceResourcesResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProvisioningStateResponse {
    pub status: RemoteProvisioningStatusResponse,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningStatusResponse {
    NotStarted,
    InProgress {
        phase: RemoteProvisioningPhaseResponse,
    },
    Cancelling {
        phase: Option<RemoteProvisioningPhaseResponse>,
    },
    Completed,
    Failed {
        phase: Option<RemoteProvisioningPhaseResponse>,
        error: RemoteProvisioningErrorResponse,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningPhaseResponse {
    CreatingRemoteVolume,
    StartingRemoteProvisioner,
    RunningRemoteProvisioner {
        status: RemoteProvisionerStatusResponse,
    },
    CreatingRemoteEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisionerStatusResponse {
    Pending,
    Starting,
    Running,
    CleaningUp,
    Succeeded,
    Failed { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RemoteProvisioningErrorResponse {
    Provider,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceResourcesResponse {
    pub remote_volume: Option<RemoteVolumeSnapshotResponse>,
    pub remote_provisioner: Option<RemoteProvisionerSnapshotResponse>,
    pub remote_endpoint: Option<RemoteEndpointSnapshotResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteVolumeSnapshotResponse {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProvisionerSnapshotResponse {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEndpointSnapshotResponse {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub workflow_preset_id: String,
    pub remote_placement: RemotePlacementPlanInput,
}

impl From<WorkspaceCatalog> for WorkspaceCatalogResponse {
    fn from(value: WorkspaceCatalog) -> Self {
        Self {
            workspaces: value.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Workspace> for WorkspaceResponse {
    fn from(value: Workspace) -> Self {
        Self {
            id: value.id,
            workflow_preset: value.workflow_preset.into(),
            runtime: value.runtime.into(),
        }
    }
}

impl From<WorkspaceRuntime> for WorkspaceRuntimeResponse {
    fn from(value: WorkspaceRuntime) -> Self {
        match value {
            WorkspaceRuntime::Remote(remote) => Self::Remote(remote.into()),
        }
    }
}

impl From<RemoteWorkspace> for RemoteWorkspaceResponse {
    fn from(value: RemoteWorkspace) -> Self {
        Self {
            remote_placement: value.remote_placement.into(),
            remote_provisioning: value.remote_provisioning.into(),
            remote_resources: value.remote_resources.into(),
        }
    }
}

impl From<RemoteProvisioningState> for RemoteProvisioningStateResponse {
    fn from(value: RemoteProvisioningState) -> Self {
        Self {
            status: value.status.into(),
            percent: value.percent,
        }
    }
}

impl From<RemoteProvisioningStatus> for RemoteProvisioningStatusResponse {
    fn from(value: RemoteProvisioningStatus) -> Self {
        match value {
            RemoteProvisioningStatus::NotStarted => Self::NotStarted,
            RemoteProvisioningStatus::InProgress { phase } => Self::InProgress {
                phase: phase.into(),
            },
            RemoteProvisioningStatus::Cancelling { phase } => Self::Cancelling {
                phase: phase.map(Into::into),
            },
            RemoteProvisioningStatus::Completed => Self::Completed,
            RemoteProvisioningStatus::Failed { phase, error } => Self::Failed {
                phase: phase.map(Into::into),
                error: error.into(),
            },
        }
    }
}

impl From<RemoteProvisioningPhase> for RemoteProvisioningPhaseResponse {
    fn from(value: RemoteProvisioningPhase) -> Self {
        match value {
            RemoteProvisioningPhase::CreatingRemoteVolume => Self::CreatingRemoteVolume,
            RemoteProvisioningPhase::StartingRemoteProvisioner => Self::StartingRemoteProvisioner,
            RemoteProvisioningPhase::RunningRemoteProvisioner { status } => {
                Self::RunningRemoteProvisioner {
                    status: status.into(),
                }
            }
            RemoteProvisioningPhase::CreatingRemoteEndpoint => Self::CreatingRemoteEndpoint,
        }
    }
}

impl From<RemoteProvisionerStatus> for RemoteProvisionerStatusResponse {
    fn from(value: RemoteProvisionerStatus) -> Self {
        match value {
            RemoteProvisionerStatus::Pending => Self::Pending,
            RemoteProvisionerStatus::Starting => Self::Starting,
            RemoteProvisionerStatus::Running => Self::Running,
            RemoteProvisionerStatus::CleaningUp => Self::CleaningUp,
            RemoteProvisionerStatus::Succeeded => Self::Succeeded,
            RemoteProvisionerStatus::Failed { code, message } => Self::Failed { code, message },
        }
    }
}

impl From<RemoteProvisioningError> for RemoteProvisioningErrorResponse {
    fn from(value: RemoteProvisioningError) -> Self {
        match value {
            RemoteProvisioningError::Provider(_) => Self::Provider,
            RemoteProvisioningError::ProvisionerWorkerTokenMissing => {
                Self::ProvisionerWorkerTokenMissing
            }
            RemoteProvisioningError::ProvisionerWorkerTokenInvalid => {
                Self::ProvisionerWorkerTokenInvalid
            }
            RemoteProvisioningError::ProvisionerWorkerUnauthorized => {
                Self::ProvisionerWorkerUnauthorized
            }
            RemoteProvisioningError::ProvisionerWorkerUnavailable => {
                Self::ProvisionerWorkerUnavailable
            }
            RemoteProvisioningError::ProvisionerWorkerConflict => Self::ProvisionerWorkerConflict,
            RemoteProvisioningError::ProvisionerWorkerResponseInvalid => {
                Self::ProvisionerWorkerResponseInvalid
            }
            RemoteProvisioningError::ProvisionerWorkerFailed => Self::ProvisionerWorkerFailed,
            RemoteProvisioningError::ProvisionerWorkerAssetDownloadFailed => {
                Self::ProvisionerWorkerAssetDownloadFailed
            }
            RemoteProvisioningError::ProvisionerWorkerAssetAuthRequired => {
                Self::ProvisionerWorkerAssetAuthRequired
            }
            RemoteProvisioningError::ProvisionerWorkerPathValidationFailed => {
                Self::ProvisionerWorkerPathValidationFailed
            }
            RemoteProvisioningError::ProvisionerWorkerStepTimeout => {
                Self::ProvisionerWorkerStepTimeout
            }
            RemoteProvisioningError::ProvisionerWorkerUnexpectedError => {
                Self::ProvisionerWorkerUnexpectedError
            }
            RemoteProvisioningError::CancellationCleanupFailed => Self::CancellationCleanupFailed,
            RemoteProvisioningError::InvalidProvisioningState { message } => {
                Self::InvalidProvisioningState { message }
            }
        }
    }
}

impl From<RemoteWorkspaceResources> for RemoteWorkspaceResourcesResponse {
    fn from(value: RemoteWorkspaceResources) -> Self {
        Self {
            remote_volume: value.remote_volume.map(Into::into),
            remote_provisioner: value.remote_provisioner.map(Into::into),
            remote_endpoint: value.remote_endpoint.map(Into::into),
        }
    }
}

impl From<RemoteVolumeSnapshot> for RemoteVolumeSnapshotResponse {
    fn from(value: RemoteVolumeSnapshot) -> Self {
        Self { id: value.id }
    }
}

impl From<RemoteProvisionerSnapshot> for RemoteProvisionerSnapshotResponse {
    fn from(value: RemoteProvisionerSnapshot) -> Self {
        Self {
            id: value.id,
            status_url: value.status_url,
        }
    }
}

impl From<RemoteEndpointSnapshot> for RemoteEndpointSnapshotResponse {
    fn from(value: RemoteEndpointSnapshot) -> Self {
        Self {
            id: value.id,
            url: value.url,
        }
    }
}
