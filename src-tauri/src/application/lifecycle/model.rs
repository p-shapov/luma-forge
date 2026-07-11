use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::runtimes::RuntimeProgress;

use super::errors::LifecycleError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperationState {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleOperationKind {
    Provision,
    Cleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOperation {
    pub id: Uuid,
    pub workspace_id: String,
    pub kind: LifecycleOperationKind,
    pub state: LifecycleOperationState,
    pub trace_id: Uuid,
    pub progress: RuntimeProgress,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

impl LifecycleOperation {
    pub fn running(
        id: Uuid,
        workspace_id: &str,
        trace_id: Uuid,
        kind: LifecycleOperationKind,
        progress: RuntimeProgress,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            workspace_id: workspace_id.to_owned(),
            kind,
            state: LifecycleOperationState::Running,
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
    ) -> Result<(), LifecycleError> {
        self.ensure_running()?;
        self.progress = progress;
        self.updated_at = now;
        Ok(())
    }

    pub fn succeed(&mut self, now: OffsetDateTime) -> Result<(), LifecycleError> {
        self.finish(LifecycleOperationState::Succeeded, now)
    }

    pub fn fail(&mut self, now: OffsetDateTime) -> Result<(), LifecycleError> {
        self.finish(LifecycleOperationState::Failed, now)
    }

    fn finish(
        &mut self,
        state: LifecycleOperationState,
        now: OffsetDateTime,
    ) -> Result<(), LifecycleError> {
        self.ensure_running()?;
        self.state = state;
        self.updated_at = now;
        self.finished_at = Some(now);
        Ok(())
    }

    fn ensure_running(&self) -> Result<(), LifecycleError> {
        (self.state == LifecycleOperationState::Running)
            .then_some(())
            .ok_or(LifecycleError::InvalidTransition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::progress_fixture;
    use uuid::Uuid;

    #[test]
    fn running_operation_can_succeed_once_and_retains_its_step() {
        let progress = progress_fixture();
        let mut operation = LifecycleOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            Uuid::from_u128(2),
            LifecycleOperationKind::Provision,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.kind, LifecycleOperationKind::Provision);
        assert_eq!(operation.state, LifecycleOperationState::Succeeded);
        assert_eq!(operation.progress, progress);
        assert_eq!(
            operation.succeed(OffsetDateTime::UNIX_EPOCH),
            Err(LifecycleError::InvalidTransition)
        );
    }

    #[test]
    fn interrupted_operation_fails_without_changing_progress_or_trace() {
        let progress = progress_fixture();
        let mut operation = LifecycleOperation::running(
            Uuid::from_u128(1),
            "workspace-1",
            Uuid::from_u128(2),
            LifecycleOperationKind::Cleanup,
            progress,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.fail(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, LifecycleOperationState::Failed);
        assert_eq!(operation.kind, LifecycleOperationKind::Cleanup);
        assert_eq!(operation.trace_id, Uuid::from_u128(2));
        assert_eq!(operation.progress, progress);
    }
}
