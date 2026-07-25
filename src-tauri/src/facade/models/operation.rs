use serde::{Deserialize, Serialize};

use crate::application::runtimes::{
    runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
    RuntimeOperation, RuntimeOperationKind, RuntimeOperationState, RuntimeProgress,
};

use super::{mapping::timestamp, FacadeMappingError, RuntimeKindDto};

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
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
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationPageDto {
    pub operations: Vec<RuntimeOperationDto>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "runtime_operation")]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOperationEvent {
    pub operation: RuntimeOperationDto,
}

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationKindDto {
    Provision,
    Cleanup,
}

impl From<RuntimeOperationKind> for RuntimeOperationKindDto {
    fn from(value: RuntimeOperationKind) -> Self {
        match value {
            RuntimeOperationKind::Provision => Self::Provision,
            RuntimeOperationKind::Cleanup => Self::Cleanup,
        }
    }
}

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperationStateDto {
    Running,
    Succeeded,
    Failed,
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

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RunpodProvisionStepDto {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
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

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RunpodCleanupStepDto {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
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
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
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
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
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

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::application::runtimes::{
        runpod::{RunpodProgress, RunpodProvisionStep},
        RuntimeKind, RuntimeOperation, RuntimeOperationKind, RuntimeProgress,
    };

    use super::*;

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
}
