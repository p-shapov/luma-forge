use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    errors::RuntimeOperationError,
    runpod::{RunpodContractRequirements, RunpodProgress, RunpodRuntime},
};

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRef {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
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

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub enum RuntimeContractRequirements {
    Runpod(#[diagnostic(show)] RunpodContractRequirements),
}

impl RuntimeContractRequirements {
    pub fn as_runpod(&self) -> Option<&RunpodContractRequirements> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub revision: String,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub description: String,
    #[diagnostic(show)]
    pub required_volume_size_gb: u64,
    #[diagnostic(show)]
    pub requires_hugging_face_api_key: bool,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    #[diagnostic(show)]
    pub summary: WorkflowSummary,
    #[diagnostic(show)]
    pub runtime_preset_ref: CatalogRef,
    #[diagnostic(show)]
    pub contract_requirements: Vec<RuntimeContractRequirements>,
    pub model_assets: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub workflow_graph: serde_json::Value,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Runpod,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub enum RuntimeProvider {
    Runpod(#[diagnostic(show)] RunpodRuntime),
}

impl RuntimeProvider {
    pub fn kind(&self) -> RuntimeKind {
        match self {
            Self::Runpod(_) => RuntimeKind::Runpod,
        }
    }

    pub fn as_runpod(&self) -> Option<&RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }

    pub fn as_runpod_mut(&mut self) -> Option<&mut RunpodRuntime> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct Runtime {
    #[diagnostic(show)]
    pub state: RuntimeState,
    #[diagnostic(show)]
    pub provider: RuntimeProvider,
}

impl Runtime {
    pub fn kind(&self) -> RuntimeKind {
        self.provider.kind()
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProgress {
    Runpod(#[diagnostic(show)] RunpodProgress),
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOperationState {
    Running,
    Succeeded,
    Failed,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOperationKind {
    Provision,
    Cleanup,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RuntimeOperation {
    #[diagnostic(show)]
    pub id: Uuid,
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub runtime_kind: RuntimeKind,
    #[diagnostic(show)]
    pub kind: RuntimeOperationKind,
    #[diagnostic(show)]
    pub state: RuntimeOperationState,
    #[diagnostic(show)]
    pub trace_id: Option<Uuid>,
    #[diagnostic(show)]
    pub progress: RuntimeProgress,
    #[diagnostic(show)]
    pub created_at: OffsetDateTime,
    #[diagnostic(show)]
    pub updated_at: OffsetDateTime,
    #[diagnostic(show)]
    pub finished_at: Option<OffsetDateTime>,
}

impl RuntimeOperation {
    pub fn running(
        id: Uuid,
        workspace_id: &str,
        runtime_kind: RuntimeKind,
        kind: RuntimeOperationKind,
        progress: RuntimeProgress,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            workspace_id: workspace_id.to_owned(),
            runtime_kind,
            kind,
            state: RuntimeOperationState::Running,
            trace_id: crate::diagnostics::current_trace_uuid(),
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
        RunpodContractRequirements, RunpodProgress, RunpodProvisionStep, RunpodRuntime,
        RunpodRuntimeConfig,
    };

    #[test]
    fn runtime_kind_comes_from_its_provider() {
        let runtime = Runtime {
            state: RuntimeState::Provisioning,
            provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
                RunpodRuntimeConfig {
                    datacenter_id: "EU-RO-1".into(),
                    gpu_id: "gpu-1".into(),
                    volume_size_gb: 100,
                },
            )),
        };

        assert_eq!(runtime.kind(), RuntimeKind::Runpod);
    }

    #[test]
    fn runtime_unions_expose_their_runpod_values() {
        let mut provider =
            RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(RunpodRuntimeConfig {
                datacenter_id: "EU-RO-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 100,
            }));

        assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 100);
        provider.as_runpod_mut().unwrap().config.volume_size_gb = 120;
        assert_eq!(provider.as_runpod().unwrap().config.volume_size_gb, 120);

        let expected = RunpodContractRequirements {
            provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
            endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
        };
        let requirements = RuntimeContractRequirements::Runpod(expected.clone());

        assert_eq!(requirements.as_runpod(), Some(&expected));
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
            RuntimeKind::Runpod,
            RuntimeOperationKind::Provision,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.kind, RuntimeOperationKind::Provision);
        assert_eq!(operation.state, RuntimeOperationState::Succeeded);
        assert_eq!(operation.progress, progress);
        assert_eq!(operation.trace_id, None);
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
            RuntimeKind::Runpod,
            RuntimeOperationKind::Cleanup,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );
        let trace_id = Uuid::from_u128(2);
        operation.trace_id = Some(trace_id);

        operation.fail(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(operation.kind, RuntimeOperationKind::Cleanup);
        assert_eq!(operation.trace_id, Some(trace_id));
        assert_eq!(operation.progress, progress);
    }
}
