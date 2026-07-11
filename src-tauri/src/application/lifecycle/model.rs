use time::OffsetDateTime;
use uuid::Uuid;

use super::{
    errors::LifecycleError,
    progress::runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleProgress {
    Runpod(RunpodProgress),
}

impl LifecycleProgress {
    pub fn provision_step(self) -> Option<RunpodProvisionStep> {
        match self {
            Self::Runpod(RunpodProgress::Provision(step)) => Some(step),
            _ => None,
        }
    }

    pub fn cleanup_step(self) -> Option<RunpodCleanupStep> {
        match self {
            Self::Runpod(RunpodProgress::Cleanup(step)) => Some(step),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleOperation {
    pub id: Uuid,
    pub workspace_id: String,
    pub state: LifecycleOperationState,
    pub trace_id: Uuid,
    pub progress: LifecycleProgress,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

impl LifecycleOperation {
    pub fn runpod_provision(
        id: Uuid,
        workspace_id: &str,
        trace_id: Uuid,
        step: RunpodProvisionStep,
        now: OffsetDateTime,
    ) -> Self {
        Self::running(
            id,
            workspace_id,
            trace_id,
            LifecycleProgress::Runpod(RunpodProgress::Provision(step)),
            now,
        )
    }

    pub fn runpod_cleanup(
        id: Uuid,
        workspace_id: &str,
        trace_id: Uuid,
        step: RunpodCleanupStep,
        now: OffsetDateTime,
    ) -> Self {
        Self::running(
            id,
            workspace_id,
            trace_id,
            LifecycleProgress::Runpod(RunpodProgress::Cleanup(step)),
            now,
        )
    }

    fn running(
        id: Uuid,
        workspace_id: &str,
        trace_id: Uuid,
        progress: LifecycleProgress,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            workspace_id: workspace_id.to_owned(),
            state: LifecycleOperationState::Running,
            trace_id,
            progress,
            created_at: now,
            updated_at: now,
            finished_at: None,
        }
    }

    pub fn set_provision_step(
        &mut self,
        step: RunpodProvisionStep,
        now: OffsetDateTime,
    ) -> Result<(), LifecycleError> {
        self.set_progress(
            LifecycleProgress::Runpod(RunpodProgress::Provision(step)),
            now,
        )
    }

    pub fn set_cleanup_step(
        &mut self,
        step: RunpodCleanupStep,
        now: OffsetDateTime,
    ) -> Result<(), LifecycleError> {
        self.set_progress(
            LifecycleProgress::Runpod(RunpodProgress::Cleanup(step)),
            now,
        )
    }

    fn set_progress(
        &mut self,
        progress: LifecycleProgress,
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

    pub fn kind(&self) -> LifecycleOperationKind {
        match self.progress {
            LifecycleProgress::Runpod(RunpodProgress::Provision(_)) => {
                LifecycleOperationKind::Provision
            }
            LifecycleProgress::Runpod(RunpodProgress::Cleanup(_)) => {
                LifecycleOperationKind::Cleanup
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::lifecycle::progress::runpod::{RunpodCleanupStep, RunpodProvisionStep};
    use uuid::Uuid;

    #[test]
    fn running_operation_can_succeed_once_and_retains_its_step() {
        let mut operation = LifecycleOperation::runpod_provision(
            Uuid::from_u128(1),
            "workspace-1",
            Uuid::from_u128(2),
            RunpodProvisionStep::CreateNetworkVolume,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, LifecycleOperationState::Succeeded);
        assert_eq!(
            operation.progress.provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(
            operation.succeed(OffsetDateTime::UNIX_EPOCH),
            Err(LifecycleError::InvalidTransition)
        );
    }

    #[test]
    fn interrupted_operation_fails_without_changing_progress_or_trace() {
        let mut operation = LifecycleOperation::runpod_cleanup(
            Uuid::from_u128(1),
            "workspace-1",
            Uuid::from_u128(2),
            RunpodCleanupStep::DeleteEndpoint,
            OffsetDateTime::UNIX_EPOCH,
        );

        operation.fail(OffsetDateTime::UNIX_EPOCH).unwrap();

        assert_eq!(operation.state, LifecycleOperationState::Failed);
        assert_eq!(operation.trace_id, Uuid::from_u128(2));
        assert_eq!(
            operation.progress.cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
    }
}
