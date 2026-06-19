use std::sync::Arc;

use crate::{
    domain::{lifecycle_operation::LifecycleOperation, workspace::Workspace},
    shared::AppFuture,
    workspace::{WorkspaceError, WorkspaceRuntime, WorkspaceRuntimeContext},
};

use super::client::RunpodRuntimeClient;

#[derive(Clone)]
pub struct RunpodWorkspaceRuntime {
    runpod_client: Arc<dyn RunpodRuntimeClient>,
}

impl RunpodWorkspaceRuntime {
    pub fn new(runpod_client: Arc<dyn RunpodRuntimeClient>) -> Self {
        Self { runpod_client }
    }
}

impl WorkspaceRuntime for RunpodWorkspaceRuntime {
    fn provision<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::provision::provision_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn cleanup<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>> {
        Box::pin(async move {
            super::cleanup::cleanup_workspace(
                context,
                self.runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
        })
    }

    fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            super::delete::delete_workspace(context, self.runpod_client.as_ref(), workspace).await
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{lifecycle_operation::LifecycleOperationPayload, workspace::WorkspaceState},
        provider::runpod::runtime::RunpodWorkspaceRuntime,
        workspace::test_support::{
            runpod_client_with_state, runtime_context_for_test, workspace_with_runpod_resources,
        },
        workspace::WorkspaceRuntime,
    };

    #[tokio::test]
    async fn cleanup_persists_cleanup_payload() {
        let runtime = RunpodWorkspaceRuntime::new(runpod_client_with_state());
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod_resources("workspace-1");
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;

        let cleaned = runtime
            .cleanup(context.clone(), operation, workspace)
            .await
            .expect("cleanup");

        assert_eq!(cleaned.state, WorkspaceState::NotProvisioned);
        let latest = context
            .latest_operation_for_test("workspace-1")
            .await
            .expect("latest operation");
        assert!(matches!(
            latest.payload,
            Some(LifecycleOperationPayload::Cleanup(_))
        ));
    }

    #[tokio::test]
    async fn delete_removes_workspace_without_persisting_delete_payload() {
        let runtime = RunpodWorkspaceRuntime::new(runpod_client_with_state());
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod_resources("workspace-1");
        context.insert_workspace_for_test(workspace.clone()).await;

        runtime
            .delete(context.clone(), workspace)
            .await
            .expect("delete");

        assert!(context
            .find_workspace_for_test("workspace-1")
            .await
            .is_none());
        assert!(context
            .latest_operation_for_test("workspace-1")
            .await
            .is_none());
    }
}
