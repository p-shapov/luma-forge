use std::sync::Arc;

use crate::application::workspace::{ports::WorkspaceRepository, Workspace};

use super::{
    ports::RuntimeOperationRepository,
    runpod::{ProvisionRunpodRuntime, RunpodRuntimeService},
    RuntimeError, RuntimeKind, RuntimeOperation,
};

#[derive(crate::diagnostics::DiagnosticDebug)]
pub enum ProvisionRuntime {
    Runpod(#[diagnostic(show)] ProvisionRunpodRuntime),
}

pub struct RuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    operations: Arc<dyn RuntimeOperationRepository>,
    runpod: RunpodRuntimeService,
}

impl RuntimeService {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        operations: Arc<dyn RuntimeOperationRepository>,
        runpod: RunpodRuntimeService,
    ) -> Self {
        Self {
            workspaces,
            operations,
            runpod,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn start_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRuntime,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        match command {
            ProvisionRuntime::Runpod(command) => self.runpod.start_provision(command).await,
        }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn start_cleanup(
        &self,
        #[diagnostic(show)] workspace_id: &str,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        let workspace = self
            .workspaces
            .get(workspace_id)
            .await
            .map_err(|_| RuntimeError::PersistenceUnavailable)?
            .ok_or(RuntimeError::WorkspaceNotFound)?;
        let kind = workspace
            .runtime
            .as_ref()
            .map(|runtime| runtime.kind())
            .ok_or(RuntimeError::NotProvisioned)?;

        match kind {
            RuntimeKind::Runpod => self.runpod.start_cleanup(workspace).await,
        }
    }

    #[crate::diagnostics::diagnostic(show_error)]
    pub async fn recover_interrupted(&self) -> Result<(), RuntimeError> {
        let operations = self.operations.running().await?;
        let mut runpod = Vec::new();
        for operation in operations {
            match operation.runtime_kind {
                RuntimeKind::Runpod => runpod.push(operation),
            }
        }
        self.runpod.recover_interrupted(runpod).await
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::{
        runpod::test_support::{provision_command, ProvisionFakes},
        RuntimeError, RuntimeKind, RuntimeOperationKind, RuntimeOperationState, RuntimeState,
    };

    use super::ProvisionRuntime;

    #[tokio::test]
    async fn provision_dispatches_the_runpod_command() {
        let fakes = ProvisionFakes::ready();
        fakes.block_first_provider_call();

        let (workspace, operation) = fakes
            .runtime_service()
            .start_provision(ProvisionRuntime::Runpod(provision_command()))
            .await
            .unwrap();

        assert_eq!(workspace.runtime.unwrap().kind(), RuntimeKind::Runpod);
        assert_eq!(operation.runtime_kind, RuntimeKind::Runpod);
        fakes.wait_until_first_provider_call().await;
        fakes.release_first_provider_call();
    }

    #[tokio::test]
    async fn cleanup_loads_the_workspace_and_dispatches_by_attached_kind() {
        let fakes = ProvisionFakes::ready_runtime();
        fakes.block_first_provider_call();

        let (workspace, operation) = fakes
            .runtime_service()
            .start_cleanup("workspace-1")
            .await
            .unwrap();

        assert_eq!(workspace.runtime.unwrap().state, RuntimeState::CleaningUp);
        assert_eq!(operation.kind, RuntimeOperationKind::Cleanup);
        fakes.wait_until_first_provider_call().await;
        fakes.release_first_provider_call();
    }

    #[tokio::test]
    async fn cleanup_reports_a_missing_workspace() {
        let fakes = ProvisionFakes::ready_runtime();

        assert_eq!(
            fakes.runtime_service().start_cleanup("missing").await,
            Err(RuntimeError::WorkspaceNotFound)
        );
    }

    #[tokio::test]
    async fn cleanup_reports_an_unprovisioned_workspace() {
        let fakes = ProvisionFakes::without_runtime();

        assert_eq!(
            fakes.runtime_service().start_cleanup("workspace-1").await,
            Err(RuntimeError::NotProvisioned)
        );
    }

    #[tokio::test]
    async fn recovery_loads_and_groups_running_operations() {
        let fakes = ProvisionFakes::with_running_provision_and_cleanup();

        fakes.runtime_service().recover_interrupted().await.unwrap();

        assert_eq!(
            fakes.saved_states(),
            vec![
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
            ]
        );
    }
}
