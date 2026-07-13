use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::application::{
    runtimes::{
        runpod::{
            RunpodCleanupStep, RunpodPlacement, RunpodPlacementDatacenter, RunpodPlacementGpu,
            RunpodProgress, RunpodProvisionStep,
        },
        CatalogRef, Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationKind,
        RuntimeOperationState, RuntimeProgress, RuntimeProvider, RuntimeState, WorkflowSummary,
    },
    secrets::Identity,
    workspace::Workspace,
};

#[derive(
    crate::diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct PageRequest {
    #[diagnostic(show)]
    pub offset: u64,
    #[diagnostic(show)]
    pub limit: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPageDto {
    pub workflows: Vec<WorkflowDto>,
    pub total: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePageDto {
    pub workspaces: Vec<WorkspaceDto>,
    pub total: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationPageRequest {
    #[diagnostic(show)]
    pub workspace_id: Option<String>,
    #[diagnostic(show)]
    pub offset: u64,
    #[diagnostic(show)]
    pub limit: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationPageDto {
    pub operations: Vec<RuntimeOperationDto>,
    pub total: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefDto {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub revision: String,
}

impl From<CatalogRef> for CatalogRefDto {
    fn from(value: CatalogRef) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
        }
    }
}

impl From<CatalogRefDto> for CatalogRef {
    fn from(value: CatalogRefDto) -> Self {
        Self::new(value.id, value.revision)
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDto {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}

impl From<WorkflowSummary> for WorkflowDto {
    fn from(value: WorkflowSummary) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            name: value.name,
            description: value.description,
            required_volume_size_gb: value.required_volume_size_gb,
            requires_hugging_face_api_key: value.requires_hugging_face_api_key,
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    #[diagnostic(show)]
    pub workflow: CatalogRefDto,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIdRequest {
    #[diagnostic(show)]
    pub workspace_id: String,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionWorkspaceRequest {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub runtime: ProvisionRuntimeInput,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(
    tag = "runtimeKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ProvisionRuntimeInput {
    Runpod {
        #[diagnostic(show)]
        datacenter_id: String,
        #[diagnostic(show)]
        gpu_id: String,
        #[diagnostic(show)]
        volume_size_gb: u64,
    },
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOperationDto {
    pub workspace: WorkspaceDto,
    pub operation: RuntimeOperationDto,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct SetupApiKeyRequest {
    #[diagnostic(redact)]
    pub api_key: String,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub id: String,
    pub workflow: CatalogRefDto,
    pub created_at: String,
    pub runtime: Option<RuntimeDto>,
}

impl TryFrom<Workspace> for WorkspaceDto {
    type Error = FacadeMappingError;

    fn try_from(value: Workspace) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            workflow: value.workflow.into(),
            created_at: timestamp(value.created_at)?,
            runtime: value.runtime.map(Into::into),
        })
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDto {
    pub state: RuntimeStateDto,
    pub provider: RuntimeProviderDto,
}

impl From<Runtime> for RuntimeDto {
    fn from(value: Runtime) -> Self {
        Self {
            state: value.state.into(),
            provider: value.provider.into(),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(
    tag = "runtimeKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RuntimeProviderDto {
    Runpod {
        datacenter_id: String,
        gpu_id: String,
        volume_size_gb: u64,
    },
}

impl From<RuntimeProvider> for RuntimeProviderDto {
    fn from(value: RuntimeProvider) -> Self {
        match value {
            RuntimeProvider::Runpod(runtime) => Self::Runpod {
                datacenter_id: runtime.config.datacenter_id,
                gpu_id: runtime.config.gpu_id,
                volume_size_gb: runtime.config.volume_size_gb,
            },
        }
    }
}

macro_rules! simple_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(
            crate::diagnostics::DiagnosticDebug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Serialize,
            Deserialize,
            specta::Type,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}

simple_enum!(RuntimeKindDto { Runpod });
simple_enum!(RuntimeStateDto {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
});
simple_enum!(RuntimeOperationKindDto { Provision, Cleanup });
simple_enum!(RuntimeOperationStateDto {
    Running,
    Succeeded,
    Failed,
});
simple_enum!(RunpodProvisionStepDto {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
});
simple_enum!(RunpodCleanupStepDto {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
});

impl From<RuntimeKind> for RuntimeKindDto {
    fn from(value: RuntimeKind) -> Self {
        match value {
            RuntimeKind::Runpod => Self::Runpod,
        }
    }
}

impl From<RuntimeState> for RuntimeStateDto {
    fn from(value: RuntimeState) -> Self {
        match value {
            RuntimeState::Provisioning => Self::Provisioning,
            RuntimeState::Ready => Self::Ready,
            RuntimeState::CleaningUp => Self::CleaningUp,
            RuntimeState::Failed => Self::Failed,
        }
    }
}

impl From<RuntimeOperationKind> for RuntimeOperationKindDto {
    fn from(value: RuntimeOperationKind) -> Self {
        match value {
            RuntimeOperationKind::Provision => Self::Provision,
            RuntimeOperationKind::Cleanup => Self::Cleanup,
        }
    }
}

impl From<RuntimeOperationState> for RuntimeOperationStateDto {
    fn from(value: RuntimeOperationState) -> Self {
        match value {
            RuntimeOperationState::Running => Self::Running,
            RuntimeOperationState::Succeeded => Self::Succeeded,
            RuntimeOperationState::Failed => Self::Failed,
        }
    }
}

impl From<RunpodProvisionStep> for RunpodProvisionStepDto {
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

impl From<RunpodCleanupStep> for RunpodCleanupStepDto {
    fn from(value: RunpodCleanupStep) -> Self {
        match value {
            RunpodCleanupStep::DeleteEndpoint => Self::DeleteEndpoint,
            RunpodCleanupStep::DeleteTemplate => Self::DeleteTemplate,
            RunpodCleanupStep::TerminateProvisionerPod => Self::TerminateProvisionerPod,
            RunpodCleanupStep::DeleteNetworkVolume => Self::DeleteNetworkVolume,
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(tag = "progressKind", rename_all = "snake_case")]
pub enum RuntimeProgressDto {
    RunpodProvision { step: RunpodProvisionStepDto },
    RunpodCleanup { step: RunpodCleanupStepDto },
}

impl From<RuntimeProgress> for RuntimeProgressDto {
    fn from(value: RuntimeProgress) -> Self {
        match value {
            RuntimeProgress::Runpod(RunpodProgress::Provision(step)) => {
                Self::RunpodProvision { step: step.into() }
            }
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(step)) => {
                Self::RunpodCleanup { step: step.into() }
            }
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationDto {
    pub id: String,
    pub workspace_id: String,
    pub runtime_kind: RuntimeKindDto,
    pub kind: RuntimeOperationKindDto,
    pub state: RuntimeOperationStateDto,
    pub trace_id: Option<String>,
    pub progress: RuntimeProgressDto,
    pub created_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
}

impl TryFrom<RuntimeOperation> for RuntimeOperationDto {
    type Error = FacadeMappingError;

    fn try_from(value: RuntimeOperation) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.to_string(),
            workspace_id: value.workspace_id,
            runtime_kind: value.runtime_kind.into(),
            kind: value.kind.into(),
            state: value.state.into(),
            trace_id: value.trace_id.map(|id| id.to_string()),
            progress: value.progress.into(),
            created_at: timestamp(value.created_at)?,
            updated_at: timestamp(value.updated_at)?,
            finished_at: value.finished_at.map(timestamp).transpose()?,
        })
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementDto {
    pub max_volume_size_gb: u64,
    pub datacenters: Vec<RunpodPlacementDatacenterDto>,
}

impl From<RunpodPlacement> for RunpodPlacementDto {
    fn from(value: RunpodPlacement) -> Self {
        Self {
            max_volume_size_gb: value.max_volume_size_gb,
            datacenters: value.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementDatacenterDto {
    pub id: String,
    pub name: String,
    pub gpus: Vec<RunpodPlacementGpuDto>,
}

impl From<RunpodPlacementDatacenter> for RunpodPlacementDatacenterDto {
    fn from(value: RunpodPlacementDatacenter) -> Self {
        Self {
            id: value.id,
            name: value.name,
            gpus: value.gpus.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RunpodPlacementGpuDto {
    pub id: String,
    pub name: String,
    pub vram_gb: u64,
}

impl From<RunpodPlacementGpu> for RunpodPlacementGpuDto {
    fn from(value: RunpodPlacementGpu) -> Self {
        Self {
            id: value.id,
            name: value.name,
            vram_gb: value.vram_gb,
        }
    }
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    pub key_name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
}

impl From<Identity> for IdentityDto {
    fn from(value: Identity) -> Self {
        Self {
            key_name: value.key_name,
            username: value.username,
            email: value.email,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FacadeMappingError {
    #[error("timestamp cannot be represented as RFC3339")]
    InvalidTimestamp,
}

fn timestamp(value: OffsetDateTime) -> Result<String, FacadeMappingError> {
    value
        .format(&Rfc3339)
        .map_err(|_| FacadeMappingError::InvalidTimestamp)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPagination;

pub fn validate_page(request: PageRequest) -> Result<(u64, u64), InvalidPagination> {
    (1..=100)
        .contains(&request.limit)
        .then_some((request.offset, request.limit))
        .ok_or(InvalidPagination)
}

pub fn validate_operation_page(
    request: &RuntimeOperationPageRequest,
) -> Result<(u64, u64), InvalidPagination> {
    validate_page(PageRequest {
        offset: request.offset,
        limit: request.limit,
    })
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::application::{
        runtimes::{
            runpod::{
                RunpodProgress, RunpodProvisionStep, RunpodRuntime, RunpodRuntimeConfig,
                RunpodRuntimeResources,
            },
            CatalogRef, Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationKind,
            RuntimeProgress, RuntimeProvider, RuntimeState,
        },
        workspace::Workspace,
    };

    use super::*;

    fn workspace_with_runpod_resources() -> Workspace {
        Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow-1", "1"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: Some(Runtime {
                state: RuntimeState::Ready,
                provider: RuntimeProvider::Runpod(RunpodRuntime {
                    config: RunpodRuntimeConfig {
                        datacenter_id: "EU-RO-1".into(),
                        gpu_id: "gpu-1".into(),
                        volume_size_gb: 100,
                    },
                    resources: RunpodRuntimeResources {
                        endpoint_id: Some("endpoint-1".into()),
                        ..Default::default()
                    },
                }),
            }),
        }
    }

    fn running_provision_operation() -> RuntimeOperation {
        RuntimeOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            RuntimeKind::Runpod,
            RuntimeOperationKind::Provision,
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    #[test]
    fn workspace_dto_exposes_shared_state_but_omits_provider_resource_ids() {
        let dto = WorkspaceDto::try_from(workspace_with_runpod_resources()).unwrap();
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["runtime"]["state"], "ready");
        assert_eq!(json["runtime"]["provider"]["runtimeKind"], "runpod");
        assert_eq!(json["runtime"]["provider"]["volumeSizeGb"], 100);
        assert!(json["runtime"]["provider"].get("volume_size_gb").is_none());
        assert!(json["runtime"]["provider"].get("resources").is_none());
        assert!(!json.to_string().contains("endpoint-1"));
    }

    #[test]
    fn operation_dto_keeps_runtime_kind_and_valid_progress_pair() {
        let dto = RuntimeOperationDto::try_from(running_provision_operation()).unwrap();
        assert_eq!(dto.runtime_kind, RuntimeKindDto::Runpod);
        assert!(matches!(
            &dto.progress,
            RuntimeProgressDto::RunpodProvision {
                step: RunpodProvisionStepDto::CreateNetworkVolume
            }
        ));

        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["runtimeKind"], "runpod");
        assert_eq!(json["progress"]["progressKind"], "runpod_provision");
        assert_eq!(json["progress"]["step"], "create_network_volume");
    }

    #[test]
    fn pagination_rejects_zero_and_more_than_one_hundred() {
        assert_eq!(
            validate_page(PageRequest {
                offset: 0,
                limit: 0
            }),
            Err(InvalidPagination)
        );
        assert_eq!(
            validate_page(PageRequest {
                offset: 0,
                limit: 101
            }),
            Err(InvalidPagination)
        );
        assert_eq!(
            validate_page(PageRequest {
                offset: 7,
                limit: 100
            }),
            Ok((7, 100))
        );
    }

    #[test]
    fn api_key_request_redacts_the_raw_key_from_diagnostics() {
        let debug = format!(
            "{:?}",
            SetupApiKeyRequest {
                api_key: "raw-secret".into()
            }
        );

        assert!(!debug.contains("raw-secret"));
    }
}
