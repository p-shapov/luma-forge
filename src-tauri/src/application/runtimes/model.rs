use fastrace::collector::TraceId;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    errors::RuntimeOperationError,
    runpod::{RunpodContractRequirements, RunpodProgress, RunpodRuntime},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRef {
    pub id: String,
    pub revision: String,
}

impl CatalogRef {
    pub fn new(id: impl Into<String>, revision: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            revision: revision.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeContractRequirements {
    Runpod(RunpodContractRequirements),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    pub summary: WorkflowSummary,
    pub runtime_preset_ref: CatalogRef,
    pub contract_requirements: Vec<RuntimeContractRequirements>,
    pub model_assets: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub workflow_graph: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Runpod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    Runpod(RunpodRuntime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProgress {
    Runpod(RunpodProgress),
}

pub trait RuntimeModel: Clone + Send + Sync + 'static {
    fn workspace_id(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn into_runtime(self) -> Runtime;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOperationState {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOperationKind {
    Provision,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOperation {
    pub id: Uuid,
    pub workspace_id: String,
    pub kind: RuntimeOperationKind,
    pub state: RuntimeOperationState,
    pub trace_id: TraceId,
    pub progress: RuntimeProgress,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

impl RuntimeOperation {
    pub fn running(
        id: Uuid,
        workspace_id: &str,
        trace_id: TraceId,
        kind: RuntimeOperationKind,
        progress: RuntimeProgress,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            workspace_id: workspace_id.to_owned(),
            kind,
            state: RuntimeOperationState::Running,
            trace_id,
            progress,
            created_at: now,
            updated_at: now,
            finished_at: None,
        }
    }

    pub fn set_progress(
        &mut self,
        progress: RuntimeProgress,
        now: OffsetDateTime,
    ) -> Result<(), RuntimeOperationError> {
        self.ensure_running()?;
        self.progress = progress;
        self.updated_at = now;
        Ok(())
    }

    pub fn succeed(&mut self, now: OffsetDateTime) -> Result<(), RuntimeOperationError> {
        self.finish(RuntimeOperationState::Succeeded, now)
    }

    pub fn fail(&mut self, now: OffsetDateTime) -> Result<(), RuntimeOperationError> {
        self.finish(RuntimeOperationState::Failed, now)
    }

    fn finish(
        &mut self,
        state: RuntimeOperationState,
        now: OffsetDateTime,
    ) -> Result<(), RuntimeOperationError> {
        self.ensure_running()?;
        self.state = state;
        self.updated_at = now;
        self.finished_at = Some(now);
        Ok(())
    }

    fn ensure_running(&self) -> Result<(), RuntimeOperationError> {
        (self.state == RuntimeOperationState::Running)
            .then_some(())
            .ok_or(RuntimeOperationError::InvalidTransition)
    }
}

#[cfg(test)]
pub(crate) fn progress_fixture() -> RuntimeProgress {
    RuntimeProgress::Runpod(RunpodProgress::Provision(
        super::runpod::RunpodProvisionStep::CreateNetworkVolume,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::runpod::{
        RunpodProgress, RunpodProvisionStep, RunpodRuntime, RunpodRuntimeConfig,
        RunpodRuntimeResources, RunpodRuntimeState,
    };

    #[test]
    fn runpod_model_converts_without_erasing_its_provider_type() {
        let runtime = RunpodRuntime {
            workspace_id: "workspace-1".into(),
            state: RunpodRuntimeState::Ready,
            config: RunpodRuntimeConfig {
                datacenter_id: "dc-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 19,
            },
            resources: RunpodRuntimeResources::default(),
        };

        assert_eq!(runtime.workspace_id(), "workspace-1");
        assert_eq!(runtime.kind(), RuntimeKind::Runpod);
        assert_eq!(runtime.clone().into_runtime(), Runtime::Runpod(runtime));
    }

    #[test]
    fn runtime_dispatch_owns_provider_progress() {
        assert_eq!(
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
        );
    }

    #[test]
    fn running_operation_can_succeed_once_and_retains_its_step() {
        let progress = crate::application::runtimes::progress_fixture();
        let mut operation = RuntimeOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            TraceId(2),
            RuntimeOperationKind::Provision,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.kind, RuntimeOperationKind::Provision);
        assert_eq!(operation.state, RuntimeOperationState::Succeeded);
        assert_eq!(operation.progress, progress);
        assert_eq!(
            operation.succeed(OffsetDateTime::UNIX_EPOCH),
            Err(RuntimeOperationError::InvalidTransition)
        );
    }

    #[test]
    fn interrupted_operation_fails_without_changing_progress_or_trace() {
        let progress = crate::application::runtimes::progress_fixture();
        let mut operation = RuntimeOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            TraceId(2),
            RuntimeOperationKind::Cleanup,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.fail(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(operation.kind, RuntimeOperationKind::Cleanup);
        assert_eq!(operation.trace_id, TraceId(2));
        assert_eq!(operation.progress, progress);
    }
}
