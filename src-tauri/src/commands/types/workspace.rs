use serde::{Deserialize, Serialize};
use specta::Type;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::domain::{
    lifecycle_operation::{
        LifecycleCleanupPayload, LifecycleOperation, LifecycleOperationPayload,
        LifecycleOperationState, LifecycleProvisionPayload,
    },
    runpod::{RunpodCleanupStep, RunpodProvisionStep, RunpodResources, RunpodRuntime},
    workflow_preset::WorkflowReference,
    workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
};

use super::placement::RunpodPlacementPlanInput;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCatalogResponse {
    pub workspaces: Vec<WorkspaceResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: String,
    pub workflow: WorkflowReferenceResponse,
    pub state: WorkspaceStateResponse,
    pub runtime: WorkspaceRuntimeResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowReferenceResponse {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStateResponse {
    NotProvisioned,
    Provisioning,
    Ready,
    CleaningUp,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum WorkspaceRuntimeResponse {
    Runpod(RunpodWorkspaceResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodWorkspaceResponse {
    pub placement: RunpodPlacementPlanInput,
    pub resources: RunpodResourcesResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodResourcesResponse {
    pub volume_id: Option<String>,
    pub provisioner_id: Option<String>,
    pub endpoint_id: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateRunpodWorkspaceRequest {
    pub workflow_preset_id: String,
    pub placement: RunpodPlacementPlanInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionWorkspaceResponse {
    pub workspace: WorkspaceResponse,
    pub operation: LifecycleOperationResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CleanupWorkspaceResponse {
    pub workspace: WorkspaceResponse,
    pub operation: LifecycleOperationResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteWorkspaceResponse {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunningLifecycleOperationsResponse {
    pub operations: Vec<LifecycleOperationResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LatestLifecycleOperationResponse {
    pub operation: Option<LifecycleOperationResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleOperationResponse {
    pub operation_id: String,
    pub workspace_id: String,
    pub state: LifecycleOperationStateResponse,
    pub payload: Option<LifecycleOperationPayloadResponse>,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleOperationStateResponse {
    Running,
    Completed,
    Failed,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LifecycleOperationPayloadResponse {
    Provision(LifecycleProvisionPayloadResponse),
    Cleanup(LifecycleCleanupPayloadResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum LifecycleProvisionPayloadResponse {
    Runpod {
        step: Option<RunpodProvisionStepResponse>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum LifecycleCleanupPayloadResponse {
    Runpod {
        step: Option<RunpodCleanupStepResponse>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionStepResponse {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunpodCleanupStepResponse {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleOperationChangedEvent {
    pub workspace_id: String,
    pub operation_id: String,
    pub trace_id: String,
    pub operation: LifecycleOperationResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChangedEvent {
    pub workspace_id: String,
    pub workspace: WorkspaceResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeletedEvent {
    pub workspace_id: String,
}

impl From<Workspace> for WorkspaceResponse {
    fn from(workspace: Workspace) -> Self {
        Self {
            id: workspace.id,
            workflow: workspace.workflow.into(),
            state: workspace.state.into(),
            runtime: workspace.runtime.into(),
        }
    }
}

impl From<crate::domain::workspace::WorkspaceCatalog> for WorkspaceCatalogResponse {
    fn from(value: crate::domain::workspace::WorkspaceCatalog) -> Self {
        Self {
            workspaces: value.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WorkflowReference> for WorkflowReferenceResponse {
    fn from(value: WorkflowReference) -> Self {
        Self {
            id: value.id,
            version: value.version,
        }
    }
}

impl From<WorkspaceState> for WorkspaceStateResponse {
    fn from(value: WorkspaceState) -> Self {
        match value {
            WorkspaceState::NotProvisioned => Self::NotProvisioned,
            WorkspaceState::Provisioning => Self::Provisioning,
            WorkspaceState::Ready => Self::Ready,
            WorkspaceState::CleaningUp => Self::CleaningUp,
            WorkspaceState::Invalid => Self::Invalid,
        }
    }
}

impl From<WorkspaceRuntime> for WorkspaceRuntimeResponse {
    fn from(value: WorkspaceRuntime) -> Self {
        match value {
            WorkspaceRuntime::Runpod(runtime) => Self::Runpod(runtime.into()),
        }
    }
}

impl From<RunpodRuntime> for RunpodWorkspaceResponse {
    fn from(value: RunpodRuntime) -> Self {
        Self {
            placement: value.placement.into(),
            resources: value.resources.into(),
        }
    }
}

impl From<RunpodResources> for RunpodResourcesResponse {
    fn from(value: RunpodResources) -> Self {
        Self {
            volume_id: value.network_volume_id,
            provisioner_id: value.provisioner_pod_id,
            endpoint_id: value.endpoint_id,
            template_id: value.template_id,
        }
    }
}

impl From<crate::workspace::ProvisionWorkspaceResponse> for ProvisionWorkspaceResponse {
    fn from(value: crate::workspace::ProvisionWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::workspace::CleanupWorkspaceResponse> for CleanupWorkspaceResponse {
    fn from(value: crate::workspace::CleanupWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::workspace::DeleteWorkspaceResponse> for DeleteWorkspaceResponse {
    fn from(value: crate::workspace::DeleteWorkspaceResponse) -> Self {
        Self {
            workspace_id: value.workspace_id,
        }
    }
}

impl From<LifecycleOperation> for LifecycleOperationResponse {
    fn from(value: LifecycleOperation) -> Self {
        Self {
            operation_id: value.operation_id,
            workspace_id: value.workspace_id,
            state: value.state.into(),
            payload: value.payload.map(Into::into),
            created_at: format_timestamp(value.created_at),
            updated_at: format_timestamp(value.updated_at),
            finished_at: value.finished_at.map(format_timestamp),
        }
    }
}

impl From<LifecycleOperationState> for LifecycleOperationStateResponse {
    fn from(value: LifecycleOperationState) -> Self {
        match value {
            LifecycleOperationState::Running => Self::Running,
            LifecycleOperationState::Completed => Self::Completed,
            LifecycleOperationState::Failed => Self::Failed,
            LifecycleOperationState::Stale => Self::Stale,
        }
    }
}

impl From<LifecycleOperationPayload> for LifecycleOperationPayloadResponse {
    fn from(value: LifecycleOperationPayload) -> Self {
        match value {
            LifecycleOperationPayload::Provision(payload) => Self::Provision(payload.into()),
            LifecycleOperationPayload::Cleanup(payload) => Self::Cleanup(payload.into()),
        }
    }
}

impl From<LifecycleProvisionPayload> for LifecycleProvisionPayloadResponse {
    fn from(value: LifecycleProvisionPayload) -> Self {
        match value {
            LifecycleProvisionPayload::Runpod(payload) => Self::Runpod {
                step: payload.step.map(Into::into),
            },
        }
    }
}

impl From<LifecycleCleanupPayload> for LifecycleCleanupPayloadResponse {
    fn from(value: LifecycleCleanupPayload) -> Self {
        match value {
            LifecycleCleanupPayload::Runpod(payload) => Self::Runpod {
                step: payload.step.map(Into::into),
            },
        }
    }
}

impl From<RunpodProvisionStep> for RunpodProvisionStepResponse {
    fn from(value: RunpodProvisionStep) -> Self {
        match value {
            RunpodProvisionStep::CreateNetworkVolume => Self::CreateNetworkVolume,
            RunpodProvisionStep::StartProvisionerPod => Self::StartProvisionerPod,
            RunpodProvisionStep::PollProvisioner => Self::PollProvisioner,
            RunpodProvisionStep::TerminateProvisionerPod => Self::TerminateProvisionerPod,
            RunpodProvisionStep::CreateTemplate => Self::CreateTemplate,
            RunpodProvisionStep::CreateEndpoint => Self::CreateEndpoint,
        }
    }
}

impl From<RunpodCleanupStep> for RunpodCleanupStepResponse {
    fn from(value: RunpodCleanupStep) -> Self {
        match value {
            RunpodCleanupStep::DeleteEndpoint => Self::DeleteEndpoint,
            RunpodCleanupStep::DeleteTemplate => Self::DeleteTemplate,
            RunpodCleanupStep::TerminateProvisionerPod => Self::TerminateProvisionerPod,
            RunpodCleanupStep::DeleteNetworkVolume => Self::DeleteNetworkVolume,
        }
    }
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
