use serde::{Deserialize, Serialize};
use specta::Type;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::domain::{
    lifecycle_operation::{
        LifecycleCleanupPayload, LifecycleOperation, LifecycleOperationPayload,
        LifecycleOperationState, LifecycleProvisionPayload,
    },
    runpod::{
        RunpodCleanupStep, RunpodLifecycleCleanupPayload, RunpodLifecycleProvisionPayload,
        RunpodProvisionStep, RunpodResources, RunpodRuntime,
    },
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
    Ready,
    CleanupRequired,
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
    Runpod(RunpodLifecycleProvisionPayloadResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum LifecycleCleanupPayloadResponse {
    Runpod(RunpodLifecycleCleanupPayloadResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodLifecycleProvisionPayloadResponse {
    pub step: Option<RunpodProvisionStepResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodLifecycleCleanupPayloadResponse {
    pub step: Option<RunpodCleanupStepResponse>,
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
    pub diagnostic_id: Option<String>,
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
            WorkspaceState::CleanupRequired => Self::CleanupRequired,
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
            LifecycleProvisionPayload::Runpod(payload) => Self::Runpod(payload.into()),
        }
    }
}

impl From<LifecycleCleanupPayload> for LifecycleCleanupPayloadResponse {
    fn from(value: LifecycleCleanupPayload) -> Self {
        match value {
            LifecycleCleanupPayload::Runpod(payload) => Self::Runpod(payload.into()),
        }
    }
}

impl From<RunpodLifecycleProvisionPayload> for RunpodLifecycleProvisionPayloadResponse {
    fn from(value: RunpodLifecycleProvisionPayload) -> Self {
        Self {
            step: value.step.map(Into::into),
        }
    }
}

impl From<RunpodLifecycleCleanupPayload> for RunpodLifecycleCleanupPayloadResponse {
    fn from(value: RunpodLifecycleCleanupPayload) -> Self {
        Self {
            step: value.step.map(Into::into),
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

#[cfg(test)]
mod tests {
    use super::{
        CreateRunpodWorkspaceRequest, LifecycleCleanupPayloadResponse,
        LifecycleOperationChangedEvent, LifecycleOperationPayloadResponse,
        LifecycleOperationResponse, LifecycleOperationStateResponse,
        LifecycleProvisionPayloadResponse, RunpodCleanupStepResponse,
        RunpodLifecycleCleanupPayloadResponse, RunpodLifecycleProvisionPayloadResponse,
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
            },
            resources: RunpodResourcesResponse {
                volume_id: None,
                provisioner_id: None,
                endpoint_id: Some("endpoint".to_string()),
                template_id: Some("template".to_string()),
            },
        });

        let json = serde_json::to_value(&response).expect("runtime json");

        assert_eq!(json["runtimeType"], "runpod");
        assert!(json["placement"].get("gpuCloudProviderId").is_none());
        assert_eq!(json["resources"]["endpointId"], "endpoint");
        assert_eq!(json["resources"]["templateId"], "template");
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
        let response = LifecycleOperationPayloadResponse::Provision(
            LifecycleProvisionPayloadResponse::Runpod(RunpodLifecycleProvisionPayloadResponse {
                step: Some(RunpodProvisionStepResponse::CreateNetworkVolume),
            }),
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""runtimeType":"runpod""#));
        assert!(json.contains(r#""operation":"provision""#));
        assert!(json.contains(r#""step":"create_network_volume""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_create_template_step() {
        let response = LifecycleOperationPayloadResponse::Provision(
            LifecycleProvisionPayloadResponse::Runpod(RunpodLifecycleProvisionPayloadResponse {
                step: Some(RunpodProvisionStepResponse::CreateTemplate),
            }),
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""step":"create_template""#));
    }

    #[test]
    fn lifecycle_operation_response_serializes_cleanup_delete_template_step() {
        let response = LifecycleOperationPayloadResponse::Cleanup(
            LifecycleCleanupPayloadResponse::Runpod(RunpodLifecycleCleanupPayloadResponse {
                step: Some(RunpodCleanupStepResponse::DeleteTemplate),
            }),
        );

        let json = serde_json::to_string(&response).expect("payload json");

        assert!(json.contains(r#""operation":"cleanup""#));
        assert!(json.contains(r#""step":"delete_template""#));
    }

    #[test]
    fn lifecycle_operation_changed_event_serializes_diagnostic_id_on_envelope() {
        let event = LifecycleOperationChangedEvent {
            workspace_id: "workspace-1".to_string(),
            operation_id: "operation-1".to_string(),
            diagnostic_id: Some("diag-123".to_string()),
            operation: lifecycle_operation_response(),
        };

        let json = serde_json::to_value(&event).expect("event json");

        assert_eq!(json["diagnosticId"], "diag-123");
        assert_eq!(json["operation"]["operationId"], "operation-1");
        assert!(json["operation"].get("diagnosticId").is_none());
        assert!(json["operation"].get("error").is_none());
    }

    #[test]
    fn lifecycle_operation_changed_event_serializes_null_diagnostic_id_when_absent() {
        let event = LifecycleOperationChangedEvent {
            workspace_id: "workspace-1".to_string(),
            operation_id: "operation-1".to_string(),
            diagnostic_id: None,
            operation: lifecycle_operation_response(),
        };

        let json = serde_json::to_value(&event).expect("event json");

        assert!(json["diagnosticId"].is_null());
        assert!(json["operation"].get("diagnosticId").is_none());
    }

    fn lifecycle_operation_response() -> LifecycleOperationResponse {
        LifecycleOperationResponse {
            operation_id: "operation-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationStateResponse::Failed,
            payload: Some(LifecycleOperationPayloadResponse::Provision(
                LifecycleProvisionPayloadResponse::Runpod(
                    RunpodLifecycleProvisionPayloadResponse { step: None },
                ),
            )),
            created_at: "2026-06-13T00:00:00Z".to_string(),
            updated_at: "2026-06-13T00:00:01Z".to_string(),
            finished_at: Some("2026-06-13T00:00:01Z".to_string()),
        }
    }
}
