use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationId, LifecycleOperationState},
        provisioned_remote::{ProvisionedRemoteDeleteStep, ProvisionedRemoteLifecycleError},
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
        errors::ProvisionedRemoteError,
        events::{ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
        provider::{DeleteEndpointParams, DeleteVolumeParams, TerminateProvisionerParams},
        registry::ProvisionedRemoteProviderRegistry,
        service::map_workspace_catalog_error,
    },
    cleanup::lifecycle_error_for,
    helpers::{
        load_running_operation, map_lifecycle_journal_error, mark_operation_state,
        mark_running_step, persist_workspace,
    },
};

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    provider_registry: &ProvisionedRemoteProviderRegistry,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
) -> Result<Option<LifecycleOperation>, ProvisionedRemoteError>
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
                ProvisionedRemoteDeleteStep::DeleteLocalWorkspace,
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
                ProvisionedRemoteDeleteStep::DeleteEndpoint,
                Some(ProvisionedRemoteLifecycleError::InvalidRuntimeState),
            )
            .await?;
            return Ok(None);
        }
    };
    let mut failed_step = ProvisionedRemoteDeleteStep::DeleteEndpoint;

    let result = async {
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        let provider = if runtime.resources.is_empty() {
            None
        } else {
            Some(provider_registry.for_provider(runtime.provider_id())?)
        };
        if let Some(endpoint) = runtime.resources.endpoint.clone() {
            failed_step = ProvisionedRemoteDeleteStep::DeleteEndpoint;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("provider should exist when endpoint exists")
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id,
                })
                .await
            {
                Ok(()) | Err(ProvisionedRemoteError::RemoteEndpointNotFound) => {
                    let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
                    runtime.resources.endpoint = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        if let Some(provisioner) = runtime.resources.provisioner.clone() {
            failed_step = ProvisionedRemoteDeleteStep::TerminateProvisioner;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("provider should exist when provisioner exists")
                .terminate_provisioner(TerminateProvisionerParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id,
                })
                .await
            {
                Ok(()) | Err(ProvisionedRemoteError::RemoteProvisionerNotFound) => {
                    let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
                    runtime.resources.provisioner = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        if let Some(volume) = runtime.resources.volume.clone() {
            failed_step = ProvisionedRemoteDeleteStep::DeleteVolume;
            mark_running_step(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step.clone(),
                None,
            )
            .await?;
            match provider
                .expect("provider should exist when volume exists")
                .delete_volume(DeleteVolumeParams {
                    workspace_id: workspace.id.clone(),
                    volume_id: volume.id,
                })
                .await
            {
                Ok(()) | Err(ProvisionedRemoteError::RemoteVolumeNotFound) => {
                    let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
                    runtime.resources.volume = None;
                    workspace =
                        persist_workspace(workspace_repository, event_sink, &workspace).await?;
                }
                Err(error) => return Err(error),
            }
        }

        failed_step = ProvisionedRemoteDeleteStep::DeleteLocalWorkspace;
        mark_running_step(
            lifecycle_journal,
            event_sink,
            &operation,
            failed_step.clone(),
            None,
        )
        .await?;
        Ok::<(), ProvisionedRemoteError>(())
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
                let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
                workspace.state = if runtime.resources.is_empty() {
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
            event_sink.emit(ProvisionedRemoteEvent::WorkspaceDeleted {
                workspace_id: workspace.id.clone(),
            });
            Ok(Some(completed_operation))
        }
        Err(error) => {
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
            workspace.state = if runtime.resources.is_empty() {
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
