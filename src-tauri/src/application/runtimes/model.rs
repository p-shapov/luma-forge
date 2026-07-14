use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::workspace::Workspace;

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

impl RuntimeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runpod => "runpod",
        }
    }
}

impl std::str::FromStr for RuntimeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "runpod" => Ok(Self::Runpod),
            _ => Err(()),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", content = "payload", deny_unknown_fields)]
pub enum RuntimeProvider {
    #[serde(rename = "runpod")]
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

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "provider", content = "payload", deny_unknown_fields)]
pub enum RuntimeProgress {
    #[serde(rename = "runpod")]
    Runpod(#[diagnostic(show)] RunpodProgress),
}

impl RuntimeProgress {
    pub fn runtime_kind(self) -> RuntimeKind {
        match self {
            Self::Runpod(_) => RuntimeKind::Runpod,
        }
    }

    pub fn operation_kind(self) -> RuntimeOperationKind {
        match self {
            Self::Runpod(progress) => progress.operation_kind(),
        }
    }
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
    pub fn validate_progress(&self) -> Result<(), RuntimeOperationError> {
        (self.runtime_kind == self.progress.runtime_kind()
            && self.kind == self.progress.operation_kind())
        .then_some(())
        .ok_or(RuntimeOperationError::InvalidTransition)
    }

    pub fn validate_transition(&self, workspace: &Workspace) -> Result<(), RuntimeOperationError> {
        self.validate_progress()?;
        (workspace.id == self.workspace_id
            && match &workspace.runtime {
                Some(runtime) => runtime.kind() == self.runtime_kind,
                None => {
                    self.kind == RuntimeOperationKind::Cleanup
                        && self.state == RuntimeOperationState::Succeeded
                }
            })
        .then_some(())
        .ok_or(RuntimeOperationError::InvalidTransition)
    }

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
    use crate::application::{
        runtimes::runpod::{
            RunpodCleanupStep, RunpodContractRequirements, RunpodProgress, RunpodProvisionStep,
            RunpodRuntime, RunpodRuntimeConfig,
        },
        workspace::Workspace,
    };

    fn provider_payload_fixture() -> RuntimeProvider {
        let mut runtime = RunpodRuntime::new_provisioning(RunpodRuntimeConfig {
            datacenter_id: "EU-RO-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb: 100,
        });
        runtime.resources.network_volume_id = Some("network-volume-1".into());
        runtime.resources.template_id = Some("template-1".into());
        RuntimeProvider::Runpod(runtime)
    }

    #[test]
    fn runtime_kind_uses_the_pinned_neutral_identifier() {
        assert_eq!(RuntimeKind::Runpod.as_str(), "runpod");
        assert_eq!("runpod".parse::<RuntimeKind>(), Ok(RuntimeKind::Runpod));
        assert_eq!("Runpod".parse::<RuntimeKind>(), Err(()));
    }

    #[test]
    fn provider_payload_is_tagged_round_trippable_and_strict() {
        let provider = provider_payload_fixture();
        let value = serde_json::to_value(&provider).unwrap();

        assert_eq!(value["provider"], "runpod");
        assert_eq!(value["payload"]["config"]["datacenter_id"], "EU-RO-1");
        assert_eq!(value["payload"]["resources"]["template_id"], "template-1");
        assert_eq!(
            serde_json::from_value::<RuntimeProvider>(value.clone()).unwrap(),
            provider
        );

        let mut unknown_field = value.clone();
        unknown_field["payload"]["config"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RuntimeProvider>(unknown_field).is_err());

        let mut invalid_type = value;
        invalid_type["payload"]["config"]["volume_size_gb"] = serde_json::json!("100");
        assert!(serde_json::from_value::<RuntimeProvider>(invalid_type).is_err());
        assert!(
            serde_json::from_value::<RuntimeProvider>(serde_json::json!({
                "provider": "unknown",
                "payload": {}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<RuntimeProvider>(serde_json::json!({
                "provider": "runpod",
                "config": {}
            }))
            .is_err()
        );
    }

    #[test]
    fn progress_payload_is_tagged_round_trippable_and_strict() {
        let progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
            RunpodProvisionStep::CreateNetworkVolume,
        ));
        let value = serde_json::to_value(progress).unwrap();

        assert_eq!(value["provider"], "runpod");
        assert_eq!(value["payload"]["operation"], "provision");
        assert_eq!(value["payload"]["step"], "create_network_volume");
        assert_eq!(
            serde_json::from_value::<RuntimeProgress>(value.clone()).unwrap(),
            progress
        );

        let mut unknown_field = value;
        unknown_field["payload"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RuntimeProgress>(unknown_field).is_err());
        assert!(
            serde_json::from_value::<RuntimeProgress>(serde_json::json!({
                "provider": "runpod",
                "payload": {
                    "operation": "provision",
                    "step": "unknown"
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn runtime_operation_validates_provider_neutral_transition_invariants() {
        let mut workspace = Workspace {
            id: "workspace-1".into(),
            workflow: CatalogRef::new("workflow-1", "1"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: Some(Runtime {
                state: RuntimeState::CleaningUp,
                provider: provider_payload_fixture(),
            }),
        };
        let mut operation = RuntimeOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            RuntimeKind::Runpod,
            RuntimeOperationKind::Cleanup,
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint)),
            OffsetDateTime::UNIX_EPOCH,
        );

        assert_eq!(operation.validate_progress(), Ok(()));
        assert_eq!(operation.validate_transition(&workspace), Ok(()));

        operation.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
            RunpodProvisionStep::CreateNetworkVolume,
        ));
        assert_eq!(
            operation.validate_progress(),
            Err(RuntimeOperationError::InvalidTransition)
        );
        operation.progress =
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint));

        workspace.id = "workspace-2".into();
        assert_eq!(
            operation.validate_transition(&workspace),
            Err(RuntimeOperationError::InvalidTransition)
        );
        workspace.id = "workspace-1".into();
        workspace.runtime = None;
        assert_eq!(
            operation.validate_transition(&workspace),
            Err(RuntimeOperationError::InvalidTransition)
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(operation.validate_transition(&workspace), Ok(()));
    }

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
