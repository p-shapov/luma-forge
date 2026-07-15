use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    runtimes::{runpod::RunpodProgress, RuntimeOperationError},
    workspace::Workspace,
};

use super::RuntimeKind;

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
        crate::application::runtimes::runpod::RunpodProvisionStep::CreateNetworkVolume,
    ))
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::application::runtimes::{
        runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
        CatalogRef, Runtime, RuntimeOperationError, RuntimeState,
    };

    use super::super::runtime::provider_payload_fixture;
    use super::*;

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
