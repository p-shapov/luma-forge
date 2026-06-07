use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::workspace::{
    ProvisionedRemoteComputeEndpointSnapshot, ProvisionedRemoteComputeProvisionerSnapshot,
    ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
    ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningState,
    ProvisionedRemoteComputeProvisioningStatus, ProvisionedRemoteComputeResources,
    ProvisionedRemoteComputeVolumeSnapshot, ProvisionedRemoteComputeWorkspace, Workspace,
    WorkspaceCatalog, WorkspaceRuntime,
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
    ProvisionedRemoteCompute(ProvisionedRemoteComputeWorkspaceResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedRemoteComputeWorkspaceResponse {
    pub remote_placement: RemotePlacementPlanInput,
    pub provisioning: ProvisionedRemoteComputeProvisioningStateResponse,
    pub resources: ProvisionedRemoteComputeResourcesResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedRemoteComputeProvisioningStateResponse {
    pub status: ProvisionedRemoteComputeProvisioningStatusResponse,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningStatusResponse {
    NotStarted,
    InProgress {
        phase: ProvisionedRemoteComputeProvisioningPhaseResponse,
    },
    Cancelling {
        phase: Option<ProvisionedRemoteComputeProvisioningPhaseResponse>,
    },
    Completed,
    Failed {
        phase: Option<ProvisionedRemoteComputeProvisioningPhaseResponse>,
        error: ProvisionedRemoteComputeProvisioningErrorResponse,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningPhaseResponse {
    CreatingRemoteVolume,
    StartingRemoteProvisioner,
    RunningRemoteProvisioner {
        status: ProvisionedRemoteComputeProvisionerStatusResponse,
    },
    CreatingRemoteEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisionerStatusResponse {
    Pending,
    Starting,
    Running,
    CleaningUp,
    Succeeded,
    Failed { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteComputeProvisioningErrorResponse {
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
pub struct ProvisionedRemoteComputeResourcesResponse {
    pub volume: Option<ProvisionedRemoteComputeVolumeSnapshotResponse>,
    pub provisioner: Option<ProvisionedRemoteComputeProvisionerSnapshotResponse>,
    pub endpoint: Option<ProvisionedRemoteComputeEndpointSnapshotResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedRemoteComputeVolumeSnapshotResponse {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedRemoteComputeProvisionerSnapshotResponse {
    pub id: String,
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedRemoteComputeEndpointSnapshotResponse {
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
            WorkspaceRuntime::ProvisionedRemoteCompute(remote) => {
                Self::ProvisionedRemoteCompute(remote.into())
            }
        }
    }
}

impl From<ProvisionedRemoteComputeWorkspace> for ProvisionedRemoteComputeWorkspaceResponse {
    fn from(value: ProvisionedRemoteComputeWorkspace) -> Self {
        Self {
            remote_placement: value.remote_placement.into(),
            provisioning: value.provisioning.into(),
            resources: value.resources.into(),
        }
    }
}

impl From<ProvisionedRemoteComputeProvisioningState>
    for ProvisionedRemoteComputeProvisioningStateResponse
{
    fn from(value: ProvisionedRemoteComputeProvisioningState) -> Self {
        Self {
            status: value.status.into(),
            percent: value.percent,
        }
    }
}

impl From<ProvisionedRemoteComputeProvisioningStatus>
    for ProvisionedRemoteComputeProvisioningStatusResponse
{
    fn from(value: ProvisionedRemoteComputeProvisioningStatus) -> Self {
        match value {
            ProvisionedRemoteComputeProvisioningStatus::NotStarted => Self::NotStarted,
            ProvisionedRemoteComputeProvisioningStatus::InProgress { phase } => Self::InProgress {
                phase: phase.into(),
            },
            ProvisionedRemoteComputeProvisioningStatus::Cancelling { phase } => Self::Cancelling {
                phase: phase.map(Into::into),
            },
            ProvisionedRemoteComputeProvisioningStatus::Completed => Self::Completed,
            ProvisionedRemoteComputeProvisioningStatus::Failed { phase, error } => Self::Failed {
                phase: phase.map(Into::into),
                error: error.into(),
            },
        }
    }
}

impl From<ProvisionedRemoteComputeProvisioningPhase>
    for ProvisionedRemoteComputeProvisioningPhaseResponse
{
    fn from(value: ProvisionedRemoteComputeProvisioningPhase) -> Self {
        match value {
            ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume => {
                Self::CreatingRemoteVolume
            }
            ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner => {
                Self::StartingRemoteProvisioner
            }
            ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status } => {
                Self::RunningRemoteProvisioner {
                    status: status.into(),
                }
            }
            ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint => {
                Self::CreatingRemoteEndpoint
            }
        }
    }
}

impl From<ProvisionedRemoteComputeProvisionerStatus>
    for ProvisionedRemoteComputeProvisionerStatusResponse
{
    fn from(value: ProvisionedRemoteComputeProvisionerStatus) -> Self {
        match value {
            ProvisionedRemoteComputeProvisionerStatus::Pending => Self::Pending,
            ProvisionedRemoteComputeProvisionerStatus::Starting => Self::Starting,
            ProvisionedRemoteComputeProvisionerStatus::Running => Self::Running,
            ProvisionedRemoteComputeProvisionerStatus::CleaningUp => Self::CleaningUp,
            ProvisionedRemoteComputeProvisionerStatus::Succeeded => Self::Succeeded,
            ProvisionedRemoteComputeProvisionerStatus::Failed { code, message } => {
                Self::Failed { code, message }
            }
        }
    }
}

impl From<ProvisionedRemoteComputeProvisioningError>
    for ProvisionedRemoteComputeProvisioningErrorResponse
{
    fn from(value: ProvisionedRemoteComputeProvisioningError) -> Self {
        match value {
            ProvisionedRemoteComputeProvisioningError::Provider(_) => Self::Provider,
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerTokenMissing => {
                Self::ProvisionerWorkerTokenMissing
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerTokenInvalid => {
                Self::ProvisionerWorkerTokenInvalid
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnauthorized => {
                Self::ProvisionerWorkerUnauthorized
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnavailable => {
                Self::ProvisionerWorkerUnavailable
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerConflict => {
                Self::ProvisionerWorkerConflict
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerResponseInvalid => {
                Self::ProvisionerWorkerResponseInvalid
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed => {
                Self::ProvisionerWorkerFailed
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerAssetDownloadFailed => {
                Self::ProvisionerWorkerAssetDownloadFailed
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerAssetAuthRequired => {
                Self::ProvisionerWorkerAssetAuthRequired
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerPathValidationFailed => {
                Self::ProvisionerWorkerPathValidationFailed
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerStepTimeout => {
                Self::ProvisionerWorkerStepTimeout
            }
            ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerUnexpectedError => {
                Self::ProvisionerWorkerUnexpectedError
            }
            ProvisionedRemoteComputeProvisioningError::CancellationCleanupFailed => {
                Self::CancellationCleanupFailed
            }
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState { message } => {
                Self::InvalidProvisioningState { message }
            }
        }
    }
}

impl From<ProvisionedRemoteComputeResources> for ProvisionedRemoteComputeResourcesResponse {
    fn from(value: ProvisionedRemoteComputeResources) -> Self {
        Self {
            volume: value.volume.map(Into::into),
            provisioner: value.provisioner.map(Into::into),
            endpoint: value.endpoint.map(Into::into),
        }
    }
}

impl From<ProvisionedRemoteComputeVolumeSnapshot>
    for ProvisionedRemoteComputeVolumeSnapshotResponse
{
    fn from(value: ProvisionedRemoteComputeVolumeSnapshot) -> Self {
        Self { id: value.id }
    }
}

impl From<ProvisionedRemoteComputeProvisionerSnapshot>
    for ProvisionedRemoteComputeProvisionerSnapshotResponse
{
    fn from(value: ProvisionedRemoteComputeProvisionerSnapshot) -> Self {
        Self {
            id: value.id,
            status_url: value.status_url,
        }
    }
}

impl From<ProvisionedRemoteComputeEndpointSnapshot>
    for ProvisionedRemoteComputeEndpointSnapshotResponse
{
    fn from(value: ProvisionedRemoteComputeEndpointSnapshot) -> Self {
        Self {
            id: value.id,
            url: value.url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProvisionedRemoteComputeProvisioningStateResponse,
        ProvisionedRemoteComputeProvisioningStatusResponse,
        ProvisionedRemoteComputeResourcesResponse, ProvisionedRemoteComputeWorkspaceResponse,
        WorkspaceRuntimeResponse,
    };

    #[test]
    fn workspace_runtime_response_serializes_provisioned_remote_compute_variant() {
        let response = WorkspaceRuntimeResponse::ProvisionedRemoteCompute(
            ProvisionedRemoteComputeWorkspaceResponse {
                remote_placement: crate::commands::types::placement::RemotePlacementPlanInput {
                    gpu_cloud_provider_id:
                        crate::commands::types::provider::GpuCloudProviderIdDto::Runpod,
                    datacenter_id: "dc".to_string(),
                    gpu_id: "gpu".to_string(),
                    volume_size_bytes: 1,
                    keep_alive_limits: None,
                },
                provisioning: ProvisionedRemoteComputeProvisioningStateResponse {
                    status: ProvisionedRemoteComputeProvisioningStatusResponse::NotStarted,
                    percent: None,
                },
                resources: ProvisionedRemoteComputeResourcesResponse {
                    volume: None,
                    provisioner: None,
                    endpoint: None,
                },
            },
        );

        let json = serde_json::to_string(&response).expect("runtime json");

        assert!(json.contains(r#""runtimeType":"provisioned_remote_compute""#));
        assert!(!json.contains("remote_provisioner"));
    }
}
