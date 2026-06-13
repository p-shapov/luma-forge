use serde::{Deserialize, Serialize};
use specta::Type;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::domain::{
    lifecycle_operation::{LifecycleOperation, LifecycleOperationPayload, LifecycleOperationState},
    runpod::{
        RunpodCleanupStep, RunpodDeleteStep, RunpodLifecycleError, RunpodLifecycleOperationPayload,
        RunpodProvisionStep, RunpodProvisionerError, RunpodResources, RunpodRuntime,
        RunpodRuntimeStateError,
    },
    workflow_preset::WorkflowReference,
    workspace::{
        Workspace, WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceRuntimeInvalidReason,
        WorkspaceState,
    },
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
    Ready,
    CleanupRequired {
        reason: WorkspaceCleanupRequiredReasonResponse,
    },
    Invalid {
        reason: WorkspaceRuntimeInvalidReasonResponse,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCleanupRequiredReasonResponse {
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    OperationInterrupted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRuntimeInvalidReasonResponse {
    OperationInterrupted,
    ProvisionFailed,
    CleanupFailed,
    DeleteFailed,
    CorruptRuntimeState,
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
    pub operation: LifecycleOperationResponse,
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
    pub payload: LifecycleOperationPayloadResponse,
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
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum LifecycleOperationPayloadResponse {
    Runpod(RunpodLifecycleOperationPayloadResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum RunpodLifecycleOperationPayloadResponse {
    Provision {
        step: Option<RunpodProvisionStepResponse>,
        error: Option<RunpodLifecycleErrorResponse>,
    },
    Cleanup {
        step: Option<RunpodCleanupStepResponse>,
        error: Option<RunpodLifecycleErrorResponse>,
    },
    Delete {
        step: Option<RunpodDeleteStepResponse>,
        error: Option<RunpodLifecycleErrorResponse>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunpodDeleteStepResponse {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
    DeleteLocalWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RunpodLifecycleErrorResponse {
    AppInterrupted,
    RunpodSecretUnavailable,
    RunpodApiFailed,
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    NetworkVolumeNotFound,
    ProvisionerPodNotFound,
    EndpointNotFound,
    TemplateNotFound,
    InvalidRuntimeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleOperationChangedEvent {
    pub workspace_id: String,
    pub operation_id: String,
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
            WorkspaceState::Ready => Self::Ready,
            WorkspaceState::CleanupRequired { reason } => Self::CleanupRequired {
                reason: reason.into(),
            },
            WorkspaceState::Invalid { reason } => Self::Invalid {
                reason: reason.into(),
            },
        }
    }
}

impl From<WorkspaceCleanupRequiredReason> for WorkspaceCleanupRequiredReasonResponse {
    fn from(value: WorkspaceCleanupRequiredReason) -> Self {
        match value {
            WorkspaceCleanupRequiredReason::ProvisionFailed => Self::ProvisionFailed,
            WorkspaceCleanupRequiredReason::CleanupFailed => Self::CleanupFailed,
            WorkspaceCleanupRequiredReason::DeleteFailed => Self::DeleteFailed,
            WorkspaceCleanupRequiredReason::OperationInterrupted => Self::OperationInterrupted,
        }
    }
}

impl From<WorkspaceRuntimeInvalidReason> for WorkspaceRuntimeInvalidReasonResponse {
    fn from(value: WorkspaceRuntimeInvalidReason) -> Self {
        match value {
            WorkspaceRuntimeInvalidReason::OperationInterrupted => Self::OperationInterrupted,
            WorkspaceRuntimeInvalidReason::ProvisionFailed => Self::ProvisionFailed,
            WorkspaceRuntimeInvalidReason::CleanupFailed => Self::CleanupFailed,
            WorkspaceRuntimeInvalidReason::DeleteFailed => Self::DeleteFailed,
            WorkspaceRuntimeInvalidReason::CorruptRuntimeState => Self::CorruptRuntimeState,
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
        }
    }
}

impl From<crate::runpod_runtime::service::ProvisionWorkspaceResponse>
    for ProvisionWorkspaceResponse
{
    fn from(value: crate::runpod_runtime::service::ProvisionWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::runpod_runtime::service::CleanupWorkspaceResponse> for CleanupWorkspaceResponse {
    fn from(value: crate::runpod_runtime::service::CleanupWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::runpod_runtime::service::DeleteWorkspaceResponse> for DeleteWorkspaceResponse {
    fn from(value: crate::runpod_runtime::service::DeleteWorkspaceResponse) -> Self {
        Self {
            workspace_id: value.workspace_id,
            operation: value.operation.into(),
        }
    }
}

impl From<LifecycleOperation> for LifecycleOperationResponse {
    fn from(value: LifecycleOperation) -> Self {
        Self {
            operation_id: value.operation_id,
            workspace_id: value.workspace_id,
            state: value.state.into(),
            payload: value.payload.into(),
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
            LifecycleOperationPayload::Runpod(payload) => Self::Runpod(payload.into()),
        }
    }
}

impl From<RunpodLifecycleOperationPayload> for RunpodLifecycleOperationPayloadResponse {
    fn from(value: RunpodLifecycleOperationPayload) -> Self {
        match value {
            RunpodLifecycleOperationPayload::Provision { step, error } => Self::Provision {
                step: step.map(Into::into),
                error: error.map(Into::into),
            },
            RunpodLifecycleOperationPayload::Cleanup { step, error } => Self::Cleanup {
                step: step.map(Into::into),
                error: error.map(Into::into),
            },
            RunpodLifecycleOperationPayload::Delete { step, error } => Self::Delete {
                step: step.map(Into::into),
                error: error.map(Into::into),
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

impl From<RunpodDeleteStep> for RunpodDeleteStepResponse {
    fn from(value: RunpodDeleteStep) -> Self {
        match value {
            RunpodDeleteStep::DeleteEndpoint => Self::DeleteEndpoint,
            RunpodDeleteStep::DeleteTemplate => Self::DeleteTemplate,
            RunpodDeleteStep::TerminateProvisionerPod => Self::TerminateProvisionerPod,
            RunpodDeleteStep::DeleteNetworkVolume => Self::DeleteNetworkVolume,
            RunpodDeleteStep::DeleteLocalWorkspace => Self::DeleteLocalWorkspace,
        }
    }
}

impl From<RunpodLifecycleError> for RunpodLifecycleErrorResponse {
    fn from(value: RunpodLifecycleError) -> Self {
        match value {
            RunpodLifecycleError::AppInterrupted => Self::AppInterrupted,
            RunpodLifecycleError::RunPodApiError(_) => Self::RunpodApiFailed,
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Unavailable {
                ..
            }) => Self::ProvisionerUnavailable,
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::ResponseInvalid {
                ..
            }) => Self::ProvisionerResponseInvalid,
            RunpodLifecycleError::ProvisionerError(RunpodProvisionerError::Failed { .. }) => {
                Self::ProvisionerFailed
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingVolume) => {
                Self::NetworkVolumeNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(
                RunpodRuntimeStateError::MissingProvisionerPod,
            ) => Self::ProvisionerPodNotFound,
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingEndpoint) => {
                Self::EndpointNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::MissingTemplate) => {
                Self::TemplateNotFound
            }
            RunpodLifecycleError::InvalidRuntimeState(RunpodRuntimeStateError::Invalid {
                ..
            }) => Self::InvalidRuntimeState,
        }
    }
}

fn format_timestamp(timestamp: OffsetDateTime) -> String {
    timestamp
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        CreateRunpodWorkspaceRequest, LifecycleOperationPayloadResponse, RunpodCleanupStepResponse,
        RunpodDeleteStepResponse, RunpodLifecycleOperationPayloadResponse,
        RunpodProvisionStepResponse, RunpodResourcesResponse, RunpodWorkspaceResponse,
        WorkspaceRuntimeResponse,
    };

    #[test]
    fn create_runpod_workspace_request_serializes_workflow_preset_id_only() {
        let request = CreateRunpodWorkspaceRequest {
            workflow_preset_id: "preset".to_string(),
            placement: crate::commands::types::placement::RunpodPlacementPlanInput {
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_size_gb: 1,
                keep_alive_limits: None,
            },
        };

        let json = serde_json::to_value(&request).expect("request json");

        assert_eq!(json["workflowPresetId"], "preset");
        assert!(json.get("workflow").is_none());
        assert!(json.get("workflowRevisionVersion").is_none());
    }

    #[test]
    fn workspace_runtime_response_serializes_runpod_variant() {
        let response = WorkspaceRuntimeResponse::Runpod(RunpodWorkspaceResponse {
            placement: crate::commands::types::placement::RunpodPlacementPlanInput {
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_size_gb: 1,
                keep_alive_limits: None,
            },
            resources: RunpodResourcesResponse {
                volume_id: None,
                provisioner_id: None,
                endpoint_id: Some("endpoint".to_string()),
            },
        });

        let json = serde_json::to_value(&response).expect("runtime json");

        assert_eq!(json["runtimeType"], "runpod");
        assert!(json["placement"].get("gpuCloudProviderId").is_none());
        assert_eq!(json["resources"]["endpointId"], "endpoint");
        assert!(json["resources"].get("endpoint").is_none());
        assert_eq!(
            json.as_object()
                .expect("runtime response should be object")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "placement".to_string(),
                "resources".to_string(),
                "runtimeType".to_string()
            ]
        );
    }

    #[test]
    fn lifecycle_operation_response_serializes_provision_payload_step() {
        let response = LifecycleOperationPayloadResponse::Runpod(
            RunpodLifecycleOperationPayloadResponse::Provision {
                step: Some(RunpodProvisionStepResponse::CreateNetworkVolume),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""runtimeType":"runpod""#));
        assert!(json.contains(r#""operation":"provision""#));
        assert!(json.contains(r#""step":"create_network_volume""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_create_template_step() {
        let response = LifecycleOperationPayloadResponse::Runpod(
            RunpodLifecycleOperationPayloadResponse::Provision {
                step: Some(RunpodProvisionStepResponse::CreateTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""step":"create_template""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_cleanup_delete_template_step() {
        let response = LifecycleOperationPayloadResponse::Runpod(
            RunpodLifecycleOperationPayloadResponse::Cleanup {
                step: Some(RunpodCleanupStepResponse::DeleteTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""operation":"cleanup""#));
        assert!(json.contains(r#""step":"delete_template""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_delete_delete_template_step() {
        let response = LifecycleOperationPayloadResponse::Runpod(
            RunpodLifecycleOperationPayloadResponse::Delete {
                step: Some(RunpodDeleteStepResponse::DeleteTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""operation":"delete""#));
        assert!(json.contains(r#""step":"delete_template""#));
    }
}
