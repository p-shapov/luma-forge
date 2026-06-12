use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationId, LifecycleOperationState},
        runpod::RunpodCleanupStep,
        workspace::WorkspaceState,
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::RunpodRuntimeError, events::RunpodRuntimeEventSink, provider::RunpodRuntimeClient,
    },
    helpers::{
        invalid_runtime_state, load_running_operation, mark_operation_failed, mark_operation_state,
        mark_workspace_failed, persist_workspace, RunpodWorkspaceFailure,
    },
    resource_cleanup,
};

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    let operation = load_running_operation(lifecycle_journal, operation_id).await?;
    let mut workspace = match workspace_repository
        .find_workspace_by_id(&operation.workspace_id)
        .await
    {
        Ok(Some(workspace)) => workspace,
        Ok(None) | Err(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                RunpodCleanupStep::DeleteEndpoint,
                Some(invalid_runtime_state()),
            )
            .await?;
            return Ok(());
        }
    };
    let mut failed_step = RunpodCleanupStep::DeleteEndpoint;

    let result = async {
        resource_cleanup::delete_remote_resources(
            &mut workspace,
            workspace_repository,
            lifecycle_journal,
            &operation,
            runpod_client,
            event_sink,
            &mut failed_step,
        )
        .await?;

        workspace.state = WorkspaceState::NotProvisioned;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;
        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(()) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                failed_step.clone(),
                None,
            )
            .await?;
        }
        Err(error) => {
            mark_workspace_failed(
                &mut workspace,
                workspace_repository,
                event_sink,
                RunpodWorkspaceFailure::Cleanup,
            )
            .await?;
            mark_operation_failed(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                &error,
            )
            .await?;
        }
    }

    Ok(())
}
