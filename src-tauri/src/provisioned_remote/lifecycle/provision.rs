use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState,
        },
        provisioned_remote::{
            ProvisionedRemoteLifecycleError, ProvisionedRemoteLifecycleOperationPayload,
            ProvisionedRemoteProvisionStep, ProvisionedRemoteProvisionerStatus,
        },
        workspace::{
            Workspace, WorkspaceCleanupRequiredReason, WorkspaceRuntime,
            WorkspaceRuntimeInvalidReason, WorkspaceState,
        },
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    super::{
        contracts::ProvisionedRemoteContractResolver,
        errors::ProvisionedRemoteError,
        events::{ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
        provider::{
            CreateEndpointParams, CreateVolumeParams, GetProvisionerStatusParams,
            StartProvisionerParams, TerminateProvisionerParams,
        },
        registry::ProvisionedRemoteProviderRegistry,
        service::map_workspace_catalog_error,
    },
    coordination::LifecycleOperationRegistry,
    helpers::map_lifecycle_journal_error,
};

pub async fn run(operation_id: LifecycleOperationId, registry: LifecycleOperationRegistry) {
    registry.complete(&operation_id);
}

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
                ProvisionedRemoteProvisionStep::CreateVolume,
                Some(ProvisionedRemoteLifecycleError::InvalidRuntimeState),
            )
            .await?;
            return Ok(());
        }
    };

    let mut failed_step = ProvisionedRemoteProvisionStep::CreateVolume;
    let result = async {
        failed_step = ProvisionedRemoteProvisionStep::CreateVolume;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        let runtime_snapshot = runtime.clone();
        let contracts = ProvisionedRemoteContractResolver::resolve(&workspace, &runtime_snapshot)?;
        let provider = provider_registry.for_provider(runtime_snapshot.provider_id())?;

        mark_step(
            lifecycle_journal,
            event_sink,
            &operation,
            ProvisionedRemoteProvisionStep::CreateVolume,
            None,
        )
        .await?;
        let volume = provider
            .create_volume(CreateVolumeParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: runtime_snapshot.placement.datacenter_id.clone(),
                gpu_id: runtime_snapshot.placement.gpu_id.clone(),
                size_bytes: runtime_snapshot.placement.volume_size_bytes,
                mount_path: "/workspace".to_string(),
            })
            .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.volume = Some(volume.clone());
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        failed_step = ProvisionedRemoteProvisionStep::StartProvisioner;
        mark_step(
            lifecycle_journal,
            event_sink,
            &operation,
            ProvisionedRemoteProvisionStep::StartProvisioner,
            None,
        )
        .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        let provisioner = provider
            .start_provisioner(StartProvisionerParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: runtime.placement.datacenter_id.clone(),
                gpu_id: runtime.placement.gpu_id.clone(),
                volume_id: volume.id.clone(),
                provisioner_image_ref: contracts.provisioner_contract.id.clone(),
                mount_path: "/workspace".to_string(),
                requires_hugging_face_api_key: workspace
                    .workflow_preset
                    .requires_hugging_face_api_key,
            })
            .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.provisioner = Some(provisioner.clone());
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        let mut provisioner_failed = false;
        loop {
            failed_step = ProvisionedRemoteProvisionStep::PollProvisioner;
            mark_step(
                lifecycle_journal,
                event_sink,
                &operation,
                ProvisionedRemoteProvisionStep::PollProvisioner,
                None,
            )
            .await?;
            let status = provider
                .get_provisioner_status(GetProvisionerStatusParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id.clone(),
                    status_url: provisioner.status_url.clone(),
                })
                .await?;
            match status {
                ProvisionedRemoteProvisionerStatus::Pending
                | ProvisionedRemoteProvisionerStatus::Starting
                | ProvisionedRemoteProvisionerStatus::Running => {}
                ProvisionedRemoteProvisionerStatus::Succeeded => break,
                ProvisionedRemoteProvisionerStatus::Failed => {
                    provisioner_failed = true;
                    break;
                }
            }
        }

        failed_step = ProvisionedRemoteProvisionStep::TerminateProvisioner;
        mark_step(
            lifecycle_journal,
            event_sink,
            &operation,
            ProvisionedRemoteProvisionStep::TerminateProvisioner,
            None,
        )
        .await?;
        provider
            .terminate_provisioner(TerminateProvisionerParams {
                workspace_id: workspace.id.clone(),
                provisioner_id: provisioner.id,
            })
            .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.provisioner = None;
        workspace = persist_workspace(workspace_repository, event_sink, &workspace).await?;

        if provisioner_failed {
            return Err(ProvisionedRemoteError::ProvisionerFailed);
        }

        failed_step = ProvisionedRemoteProvisionStep::CreateEndpoint;
        mark_step(
            lifecycle_journal,
            event_sink,
            &operation,
            ProvisionedRemoteProvisionStep::CreateEndpoint,
            None,
        )
        .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        let endpoint = provider
            .create_endpoint(CreateEndpointParams {
                workspace_id: workspace.id.clone(),
                datacenter_id: runtime.placement.datacenter_id.clone(),
                gpu_id: runtime.placement.gpu_id.clone(),
                volume_id: volume.id,
                endpoint_image_ref: contracts.endpoint_contract.id.clone(),
                mount_path: "/workspace".to_string(),
                keep_alive_limits: runtime.placement.keep_alive_limits.clone(),
            })
            .await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.endpoint = Some(endpoint);
        workspace.state = WorkspaceState::Ready;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;

        Ok::<(), ProvisionedRemoteError>(())
    }
    .await;

    match result {
        Ok(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                ProvisionedRemoteProvisionStep::CreateEndpoint,
                None,
            )
            .await?;
        }
        Err(error) => {
            let lifecycle_error = lifecycle_error_for(&error);
            let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
            workspace.state = if runtime.resources.is_empty() {
                WorkspaceState::Invalid {
                    reason: WorkspaceRuntimeInvalidReason::ProvisionFailed,
                }
            } else {
                WorkspaceState::CleanupRequired {
                    reason: WorkspaceCleanupRequiredReason::ProvisionFailed,
                }
            };
            persist_workspace(workspace_repository, event_sink, &workspace).await?;
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Failed,
                failed_step,
                Some(lifecycle_error),
            )
            .await?;
        }
    }

    Ok(())
}

async fn load_running_operation<L>(
    lifecycle_journal: &L,
    operation_id: &LifecycleOperationId,
) -> Result<LifecycleOperation, ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
{
    lifecycle_journal
        .list_running()
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &String::new()))?
        .into_iter()
        .find(|operation| operation.operation_id == *operation_id)
        .ok_or(ProvisionedRemoteError::StorageUnavailable)
}

async fn mark_step<L>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    operation: &LifecycleOperation,
    step: ProvisionedRemoteProvisionStep,
    error: Option<ProvisionedRemoteLifecycleError>,
) -> Result<(), ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
{
    mark_operation_state(
        lifecycle_journal,
        event_sink,
        operation,
        LifecycleOperationState::Running,
        step,
        error,
    )
    .await
    .map(|_| ())
}

async fn mark_operation_state<L>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    operation: &LifecycleOperation,
    state: LifecycleOperationState,
    step: ProvisionedRemoteProvisionStep,
    error: Option<ProvisionedRemoteLifecycleError>,
) -> Result<LifecycleOperation, ProvisionedRemoteError>
where
    L: LifecycleJournalRepository,
{
    let payload = LifecycleOperationPayload::ProvisionedRemote(
        ProvisionedRemoteLifecycleOperationPayload::Provision {
            step: Some(step),
            error,
        },
    );
    let operation = lifecycle_journal
        .mark_state(&operation.operation_id, state, &payload)
        .await
        .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;
    event_sink.emit(ProvisionedRemoteEvent::LifecycleOperationChanged {
        workspace_id: operation.workspace_id.clone(),
        operation_id: operation.operation_id.clone(),
        operation: operation.clone(),
    });
    Ok(operation)
}

async fn persist_workspace<W>(
    workspace_repository: &W,
    event_sink: &Arc<dyn ProvisionedRemoteEventSink>,
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteError>
where
    W: WorkspaceCatalogRepository,
{
    let workspace = workspace_repository
        .update_workspace(workspace)
        .await
        .map_err(map_workspace_catalog_error)?;
    event_sink.emit(ProvisionedRemoteEvent::WorkspaceChanged {
        workspace_id: workspace.id.clone(),
        workspace: workspace.clone(),
    });
    Ok(workspace)
}

fn lifecycle_error_for(error: &ProvisionedRemoteError) -> ProvisionedRemoteLifecycleError {
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
