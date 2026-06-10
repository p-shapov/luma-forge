use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationId, LifecycleOperationState},
        provisioned_remote::{ProvisionedRemoteCleanupStep, ProvisionedRemoteLifecycleError},
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
        events::ProvisionedRemoteEventSink,
        provider::{DeleteEndpointParams, DeleteVolumeParams, TerminateProvisionerParams},
        registry::ProvisionedRemoteProviderRegistry,
    },
    helpers::{load_running_operation, mark_operation_state, mark_running_step, persist_workspace},
};

pub async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    provider_registry: &ProvisionedRemoteProviderRegistry,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
) -> Result<(), ProvisionedRemoteError>
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
                ProvisionedRemoteCleanupStep::DeleteEndpoint,
                Some(ProvisionedRemoteLifecycleError::InvalidRuntimeState),
            )
            .await?;
            return Ok(());
        }
    };
    let mut failed_step = ProvisionedRemoteCleanupStep::DeleteEndpoint;

    let result = async {
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        let provider = if runtime.resources.is_empty() {
            None
        } else {
            Some(provider_registry.for_provider(runtime.provider_id())?)
        };
        if let Some(endpoint) = runtime.resources.endpoint.clone() {
            failed_step = ProvisionedRemoteCleanupStep::DeleteEndpoint;
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
            failed_step = ProvisionedRemoteCleanupStep::TerminateProvisioner;
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
            failed_step = ProvisionedRemoteCleanupStep::DeleteVolume;
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

        workspace.state = WorkspaceState::NotProvisioned;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;
        Ok::<(), ProvisionedRemoteError>(())
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
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
            workspace.state = if runtime.resources.is_empty() {
                WorkspaceState::Invalid {
                    reason: WorkspaceRuntimeInvalidReason::CleanupFailed,
                }
            } else {
                WorkspaceState::CleanupRequired {
                    reason: WorkspaceCleanupRequiredReason::CleanupFailed,
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
        }
    }

    Ok(())
}

pub(super) fn lifecycle_error_for(
    error: &ProvisionedRemoteError,
) -> ProvisionedRemoteLifecycleError {
    match error {
        ProvisionedRemoteError::ProviderAdapterUnavailable => {
            ProvisionedRemoteLifecycleError::ProviderAdapterUnavailable
        }
        ProvisionedRemoteError::ProviderSecretUnavailable => {
            ProvisionedRemoteLifecycleError::ProviderSecretUnavailable
        }
        ProvisionedRemoteError::ProviderApiFailed(reason) => {
            ProvisionedRemoteLifecycleError::ProviderApiFailed {
                reason: reason.clone(),
            }
        }
        ProvisionedRemoteError::ProvisionerUnavailable => {
            ProvisionedRemoteLifecycleError::ProvisionerUnavailable
        }
        ProvisionedRemoteError::ProvisionerResponseInvalid => {
            ProvisionedRemoteLifecycleError::ProvisionerResponseInvalid
        }
        ProvisionedRemoteError::ProvisionerFailed => {
            ProvisionedRemoteLifecycleError::ProvisionerFailed
        }
        ProvisionedRemoteError::RemoteVolumeNotFound => {
            ProvisionedRemoteLifecycleError::RemoteVolumeNotFound
        }
        ProvisionedRemoteError::RemoteProvisionerNotFound => {
            ProvisionedRemoteLifecycleError::RemoteProvisionerNotFound
        }
        ProvisionedRemoteError::RemoteEndpointNotFound => {
            ProvisionedRemoteLifecycleError::RemoteEndpointNotFound
        }
        ProvisionedRemoteError::InvalidRuntimeState
        | ProvisionedRemoteError::WorkspaceNotFound
        | ProvisionedRemoteError::WorkspaceAlreadyExists
        | ProvisionedRemoteError::LifecycleOperationAlreadyRunning { .. }
        | ProvisionedRemoteError::StorageUnavailable => {
            ProvisionedRemoteLifecycleError::InvalidRuntimeState
        }
    }
}
