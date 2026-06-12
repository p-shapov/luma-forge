use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationId, LifecycleOperationState},
        runpod::RunpodDeleteStep,
        workspace::{
            WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceRuntimeInvalidReason,
            WorkspaceState,
        },
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        errors::RunpodRuntimeError,
        events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
        provider::RunpodRuntimeClient,
        service::map_workspace_catalog_error,
    },
    cleanup::{invalid_runtime_state, lifecycle_error_for},
    helpers::{
        load_running_operation, map_lifecycle_journal_error, mark_operation_state,
        mark_running_step, persist_workspace, runpod_resources_are_empty,
    },
};

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
) -> Result<Option<LifecycleOperation>, RunpodRuntimeError>
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
        Ok(None) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                RunpodDeleteStep::DeleteLocalWorkspace,
                None,
            )
            .await?;
            lifecycle_journal
                .delete_for_workspace(&operation.workspace_id)
                .await
                .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;
            return Ok(None);
        }
        Err(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                RunpodDeleteStep::DeleteEndpoint,
                Some(invalid_runtime_state()),
            )
            .await?;
            return Ok(None);
        }
    };
    let mut failed_step = RunpodDeleteStep::DeleteEndpoint;

    let result = async {
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        let provider = if runpod_resources_are_empty(&runtime.resources) {
            None
        } else {
            Some(runpod_client)
        };
        if let Some(endpoint_id) = runtime.resources.endpoint_id.clone() {
            failed_step = RunpodDeleteStep::DeleteEndpoint;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when endpoint exists")
                .delete_serverless_endpoint(&endpoint_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::EndpointNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.endpoint_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(template_id) = runtime.resources.template_id.clone() {
            failed_step = RunpodDeleteStep::DeleteTemplate;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when template exists")
                .delete_template(&template_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::TemplateNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.template_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(provisioner_id) = runtime.resources.provisioner_pod_id.clone() {
            failed_step = RunpodDeleteStep::TerminateProvisionerPod;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when provisioner exists")
                .terminate_provisioner_pod(&provisioner_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::ProvisionerPodNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.provisioner_pod_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if let Some(volume_id) = runtime.resources.network_volume_id.clone() {
            failed_step = RunpodDeleteStep::DeleteNetworkVolume;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("client should exist when volume exists")
                .delete_network_volume(&volume_id)
                .await
            {
                Ok(()) | Err(RunpodRuntimeError::NetworkVolumeNotFound) => {
                    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                    runtime.resources.network_volume_id = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        failed_step = RunpodDeleteStep::DeleteLocalWorkspace;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            failed_step.clone(),
            None,
        )
        .await?;
        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(()) => {
            if let Err(error) = workspace_repository
                .delete_workspace(&workspace.id)
                .await
                .map_err(map_workspace_catalog_error)
            {
                let lifecycle_error = lifecycle_error_for(&error);
                let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
                workspace.state = if runpod_resources_are_empty(&runtime.resources) {
                    WorkspaceState::Invalid {
                        reason: WorkspaceRuntimeInvalidReason::DeleteFailed,
                    }
                } else {
                    WorkspaceState::CleanupRequired {
                        reason: WorkspaceCleanupRequiredReason::DeleteFailed,
                    }
                };
                persist_workspace(workspace_repository, event_sink, &workspace).await?;
                mark_operation_state(
                    lifecycle_journal,
                    event_sink,
                    &operation,
                    LifecycleOperationState::Failed,
                    failed_step.clone(),
                    Some(lifecycle_error),
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
                None,
            )
            .await?;
            lifecycle_journal
                .delete_for_workspace(&operation.workspace_id)
                .await
                .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;
            event_sink.emit(RunpodRuntimeEvent::WorkspaceDeleted {
                workspace_id: workspace.id.clone(),
            });
            Ok(Some(completed_operation))
        }
        Err(error) => {
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
            workspace.state = if runpod_resources_are_empty(&runtime.resources) {
                WorkspaceState::Invalid {
                    reason: WorkspaceRuntimeInvalidReason::DeleteFailed,
                }
            } else {
                WorkspaceState::CleanupRequired {
                    reason: WorkspaceCleanupRequiredReason::DeleteFailed,
                }
            };
            persist_workspace(workspace_repository, event_sink, &workspace).await?;
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                failed_step.clone(),
                Some(lifecycle_error),
            )
            .await?;
            Ok(None)
        }
    }
}
