use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationId, LifecycleOperationState},
        runpod::RunpodDeleteStep,
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::{invalid_runtime_state_error, RunpodRuntimeError},
        events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
        provider::RunpodRuntimeClient,
    },
    helpers::{
        load_running_operation, mark_operation_failed, mark_operation_state, mark_running_step,
        mark_workspace_failed, RunpodWorkspaceFailure,
    },
    resource_cleanup::delete_remote_resources,
};

#[tracing::instrument(
    name = "runpod_lifecycle",
    skip_all,
    fields(
        operation_kind = "delete",
        operation_id = %operation_id,
        workspace_id = tracing::field::Empty
    )
)]
pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_catalog: &W,
    lifecycle_journal: &L,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
) -> Result<Option<LifecycleOperation>, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    let operation = load_running_operation(lifecycle_journal, operation_id).await?;
    tracing::Span::current().record(
        "workspace_id",
        tracing::field::display(&operation.workspace_id),
    );
    let mut workspace = match workspace_catalog
        .find_workspace_by_id(&operation.workspace_id)
        .await
    {
        Ok(Some(workspace)) => workspace,
        Ok(None) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                RunpodDeleteStep::DeleteLocalWorkspace,
            )
            .await?;
            lifecycle_journal
                .delete_for_workspace(&operation.workspace_id)
                .await
                .map_err(invalid_runtime_state_error)?;
            return Ok(None);
        }
        Err(error) => {
            let error = RunpodRuntimeError::from(error);
            mark_operation_failed(
                lifecycle_journal,
                event_sink,
                &operation,
                RunpodDeleteStep::DeleteEndpoint,
                &error,
            )
            .await?;
            return Ok(None);
        }
    };
    let mut failed_step = RunpodDeleteStep::DeleteEndpoint;

    let result = async {
        delete_remote_resources(
            &mut workspace,
            workspace_catalog,
            lifecycle_journal,
            &operation,
            runpod_client,
            event_sink,
            &mut failed_step,
        )
        .await?;

        failed_step = RunpodDeleteStep::DeleteLocalWorkspace;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            failed_step.clone(),
        )
        .await?;
        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(()) => {
            if let Err(error) = workspace_catalog
                .delete_workspace(&workspace.id)
                .await
                .map_err(RunpodRuntimeError::from)
            {
                mark_workspace_failed(
                    &mut workspace,
                    workspace_catalog,
                    event_sink,
                    RunpodWorkspaceFailure::Delete,
                )
                .await?;
                mark_operation_state(
                    lifecycle_journal,
                    event_sink,
                    &operation,
                    LifecycleOperationState::Failed,
                    failed_step.clone(),
                )
                .await?;
                return Err(error);
            }
            let completed_operation = mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                failed_step.clone(),
            )
            .await?;
            lifecycle_journal
                .delete_for_workspace(&operation.workspace_id)
                .await
                .map_err(invalid_runtime_state_error)?;
            event_sink.emit(RunpodRuntimeEvent::WorkspaceDeleted {
                workspace_id: workspace.id.clone(),
            });
            Ok(Some(completed_operation))
        }
        Err(error) => {
            mark_workspace_failed(
                &mut workspace,
                workspace_catalog,
                event_sink,
                RunpodWorkspaceFailure::Delete,
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
            Ok(None)
        }
    }
}
