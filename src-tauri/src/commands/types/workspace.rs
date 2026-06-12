use serde::{Deserialize, Serialize};
use specta::Type;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::domain::{
    lifecycle_operation::{LifecycleOperation, LifecycleOperationPayload, LifecycleOperationState},
    provisioned_remote::{
        ProvisionedRemoteCleanupStep, ProvisionedRemoteDeleteStep, ProvisionedRemoteLifecycleError,
        ProvisionedRemoteLifecycleOperationPayload, ProvisionedRemoteProvisionStep,
        RunpodResources, RunpodRuntime,
    },
    workflow_preset::WorkflowReference,
    workspace::{
        Workspace, WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceRuntimeInvalidReason,
        WorkspaceState,
    },
};

use super::placement::RemotePlacementPlanInput;

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
    pub placement: RemotePlacementPlanInput,
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
pub struct CreateWorkspaceRequest {
    pub workflow_preset_id: String,
    pub remote_placement: RemotePlacementPlanInput,
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
    ProvisionedRemote(ProvisionedRemoteLifecycleOperationPayloadResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ProvisionedRemoteLifecycleOperationPayloadResponse {
    Provision {
        step: Option<ProvisionedRemoteProvisionStepResponse>,
        error: Option<ProvisionedRemoteLifecycleErrorResponse>,
    },
    Cleanup {
        step: Option<ProvisionedRemoteCleanupStepResponse>,
        error: Option<ProvisionedRemoteLifecycleErrorResponse>,
    },
    Delete {
        step: Option<ProvisionedRemoteDeleteStepResponse>,
        error: Option<ProvisionedRemoteLifecycleErrorResponse>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteProvisionStepResponse {
    CreateVolume,
    StartProvisioner,
    PollProvisioner,
    TerminateProvisioner,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteCleanupStepResponse {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisioner,
    DeleteVolume,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteDeleteStepResponse {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisioner,
    DeleteVolume,
    DeleteLocalWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionedRemoteLifecycleErrorResponse {
    AppInterrupted,
    ProviderSecretUnavailable,
    ProviderApiFailed,
    ProvisionerUnavailable,
    ProvisionerResponseInvalid,
    ProvisionerFailed,
    RemoteVolumeNotFound,
    RemoteProvisionerNotFound,
    RemoteEndpointNotFound,
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

impl From<crate::provisioned_remote::service::ProvisionWorkspaceResponse>
    for ProvisionWorkspaceResponse
{
    fn from(value: crate::provisioned_remote::service::ProvisionWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::provisioned_remote::service::CleanupWorkspaceResponse>
    for CleanupWorkspaceResponse
{
    fn from(value: crate::provisioned_remote::service::CleanupWorkspaceResponse) -> Self {
        Self {
            workspace: value.workspace.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<crate::provisioned_remote::service::DeleteWorkspaceResponse> for DeleteWorkspaceResponse {
    fn from(value: crate::provisioned_remote::service::DeleteWorkspaceResponse) -> Self {
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
            LifecycleOperationPayload::ProvisionedRemote(payload) => {
                Self::ProvisionedRemote(payload.into())
            }
        }
    }
}

impl From<ProvisionedRemoteLifecycleOperationPayload>
    for ProvisionedRemoteLifecycleOperationPayloadResponse
{
    fn from(value: ProvisionedRemoteLifecycleOperationPayload) -> Self {
        match value {
            ProvisionedRemoteLifecycleOperationPayload::Provision { step, error } => {
                Self::Provision {
                    step: step.map(Into::into),
                    error: error.map(Into::into),
                }
            }
            ProvisionedRemoteLifecycleOperationPayload::Cleanup { step, error } => Self::Cleanup {
                step: step.map(Into::into),
                error: error.map(Into::into),
            },
            ProvisionedRemoteLifecycleOperationPayload::Delete { step, error } => Self::Delete {
                step: step.map(Into::into),
                error: error.map(Into::into),
            },
        }
    }
}

impl From<ProvisionedRemoteProvisionStep> for ProvisionedRemoteProvisionStepResponse {
    fn from(value: ProvisionedRemoteProvisionStep) -> Self {
        match value {
            ProvisionedRemoteProvisionStep::CreateVolume => Self::CreateVolume,
            ProvisionedRemoteProvisionStep::StartProvisioner => Self::StartProvisioner,
            ProvisionedRemoteProvisionStep::PollProvisioner => Self::PollProvisioner,
            ProvisionedRemoteProvisionStep::TerminateProvisioner => Self::TerminateProvisioner,
            ProvisionedRemoteProvisionStep::CreateTemplate => Self::CreateTemplate,
            ProvisionedRemoteProvisionStep::CreateEndpoint => Self::CreateEndpoint,
        }
    }
}

impl From<ProvisionedRemoteCleanupStep> for ProvisionedRemoteCleanupStepResponse {
    fn from(value: ProvisionedRemoteCleanupStep) -> Self {
        match value {
            ProvisionedRemoteCleanupStep::DeleteEndpoint => Self::DeleteEndpoint,
            ProvisionedRemoteCleanupStep::DeleteTemplate => Self::DeleteTemplate,
            ProvisionedRemoteCleanupStep::TerminateProvisioner => Self::TerminateProvisioner,
            ProvisionedRemoteCleanupStep::DeleteVolume => Self::DeleteVolume,
        }
    }
}

impl From<ProvisionedRemoteDeleteStep> for ProvisionedRemoteDeleteStepResponse {
    fn from(value: ProvisionedRemoteDeleteStep) -> Self {
        match value {
            ProvisionedRemoteDeleteStep::DeleteEndpoint => Self::DeleteEndpoint,
            ProvisionedRemoteDeleteStep::DeleteTemplate => Self::DeleteTemplate,
            ProvisionedRemoteDeleteStep::TerminateProvisioner => Self::TerminateProvisioner,
            ProvisionedRemoteDeleteStep::DeleteVolume => Self::DeleteVolume,
            ProvisionedRemoteDeleteStep::DeleteLocalWorkspace => Self::DeleteLocalWorkspace,
        }
    }
}

impl From<ProvisionedRemoteLifecycleError> for ProvisionedRemoteLifecycleErrorResponse {
    fn from(value: ProvisionedRemoteLifecycleError) -> Self {
        match value {
            ProvisionedRemoteLifecycleError::AppInterrupted => Self::AppInterrupted,
            ProvisionedRemoteLifecycleError::ProviderSecretUnavailable => {
                Self::ProviderSecretUnavailable
            }
            ProvisionedRemoteLifecycleError::ProviderApiFailed { .. } => Self::ProviderApiFailed,
            ProvisionedRemoteLifecycleError::ProvisionerUnavailable => Self::ProvisionerUnavailable,
            ProvisionedRemoteLifecycleError::ProvisionerResponseInvalid => {
                Self::ProvisionerResponseInvalid
            }
            ProvisionedRemoteLifecycleError::ProvisionerFailed => Self::ProvisionerFailed,
            ProvisionedRemoteLifecycleError::RemoteVolumeNotFound => Self::RemoteVolumeNotFound,
            ProvisionedRemoteLifecycleError::RemoteProvisionerNotFound => {
                Self::RemoteProvisionerNotFound
            }
            ProvisionedRemoteLifecycleError::RemoteEndpointNotFound => Self::RemoteEndpointNotFound,
            ProvisionedRemoteLifecycleError::InvalidRuntimeState => Self::InvalidRuntimeState,
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
        CreateWorkspaceRequest, LifecycleOperationPayloadResponse,
        ProvisionedRemoteCleanupStepResponse, ProvisionedRemoteDeleteStepResponse,
        ProvisionedRemoteLifecycleOperationPayloadResponse, ProvisionedRemoteProvisionStepResponse,
        RunpodResourcesResponse, RunpodWorkspaceResponse, WorkspaceRuntimeResponse,
    };

    #[test]
    fn create_workspace_request_serializes_workflow_preset_id_only() {
        let request = CreateWorkspaceRequest {
            workflow_preset_id: "preset".to_string(),
            remote_placement: crate::commands::types::placement::RemotePlacementPlanInput {
                gpu_cloud_provider_id:
                    crate::commands::types::provider::GpuCloudProviderIdDto::Runpod,
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
            placement: crate::commands::types::placement::RemotePlacementPlanInput {
                gpu_cloud_provider_id:
                    crate::commands::types::provider::GpuCloudProviderIdDto::Runpod,
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
        assert_eq!(json["placement"]["gpuCloudProviderId"], "runpod");
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
        let response = LifecycleOperationPayloadResponse::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayloadResponse::Provision {
                step: Some(ProvisionedRemoteProvisionStepResponse::CreateVolume),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""runtimeType":"provisioned_remote""#));
        assert!(json.contains(r#""operation":"provision""#));
        assert!(json.contains(r#""step":"create_volume""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_create_template_step() {
        let response = LifecycleOperationPayloadResponse::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayloadResponse::Provision {
                step: Some(ProvisionedRemoteProvisionStepResponse::CreateTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""step":"create_template""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_cleanup_delete_template_step() {
        let response = LifecycleOperationPayloadResponse::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayloadResponse::Cleanup {
                step: Some(ProvisionedRemoteCleanupStepResponse::DeleteTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""operation":"cleanup""#));
        assert!(json.contains(r#""step":"delete_template""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_delete_delete_template_step() {
        let response = LifecycleOperationPayloadResponse::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayloadResponse::Delete {
                step: Some(ProvisionedRemoteDeleteStepResponse::DeleteTemplate),
                error: None,
            },
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""operation":"delete""#));
        assert!(json.contains(r#""step":"delete_template""#));
    }
}
