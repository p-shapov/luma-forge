use crate::domain::workspace::{
    ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
    ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
    ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
};

use super::{
    errors::ProvisionedRemoteComputeError,
    helpers::{
        failed_workspace_from_result, ignore_expected_error, reset_remote_state,
        with_cleanup_failure, with_provisioning_failure, with_status_and_resources,
    },
    provider::{
        DeleteEndpointParams, DeleteVolumeParams, ProvisionedRemoteComputeProvider,
        TerminateProvisionerParams,
    },
};

pub(crate) async fn handle_cancelling(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    provider: &dyn ProvisionedRemoteComputeProvider,
    phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    if let Some(endpoint) = remote.resources.endpoint.as_ref() {
        return match ignore_expected_error(
            provider
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteEndpointNotFound,
        ) {
            Ok(()) => Ok(with_status_and_resources(
                workspace,
                ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                    phase: Some(
                        ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                            status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                        },
                    ),
                },
                75,
                |resources| {
                    resources.endpoint = None;
                },
            )),
            Err(error) => Ok(with_cleanup_failure(
                workspace,
                Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint),
                error,
            )),
        };
    }

    if let Some(provisioner) = remote.resources.provisioner.as_ref() {
        let attempted_phase = match phase {
            Some(ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status,
            }) => {
                Some(ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status })
            }
            _ => Some(
                ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                },
            ),
        };
        return match ignore_expected_error(
            provider
                .terminate_provisioner(TerminateProvisionerParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteProvisionerNotFound,
        ) {
            Ok(()) => Ok(with_status_and_resources(
                workspace,
                ProvisionedRemoteComputeProvisioningStatus::Cancelling {
                    phase: Some(
                        ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
                    ),
                },
                25,
                |resources| {
                    resources.provisioner = None;
                },
            )),
            Err(error) => Ok(with_cleanup_failure(workspace, attempted_phase, error)),
        };
    }

    if let Some(volume) = remote.resources.volume.as_ref() {
        return match ignore_expected_error(
            provider
                .delete_volume(DeleteVolumeParams {
                    workspace_id: workspace.id.clone(),
                    volume_id: volume.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteVolumeNotFound,
        ) {
            Ok(()) => Ok(reset_remote_state(workspace)),
            Err(error) => Ok(with_cleanup_failure(
                workspace,
                Some(ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner),
                error,
            )),
        };
    }

    Ok(reset_remote_state(workspace))
}

pub(crate) fn mark_cancelling(
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &workspace.runtime;

    let ProvisionedRemoteComputeProvisioningStatus::InProgress { phase } =
        &remote.provisioning.status
    else {
        return Ok(with_provisioning_failure(
            workspace,
            None,
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: "only in-progress provisioning can be cancelled".to_string(),
            },
        ));
    };

    let mut workspace = workspace.clone();
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
    remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::Cancelling {
        phase: Some(phase.clone()),
    };
    Ok(workspace)
}

pub(crate) async fn cleanup_workspace(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    provider: &dyn ProvisionedRemoteComputeProvider,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let endpoint_cleanup = match &remote.resources.endpoint {
        Some(endpoint) => failed_workspace_from_result(
            workspace,
            provider
                .delete_endpoint(DeleteEndpointParams {
                    workspace_id: workspace.id.clone(),
                    endpoint_id: endpoint.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteEndpointNotFound,
        ),
        None => None,
    };

    if let Some(failed_workspace) = endpoint_cleanup {
        return Ok(failed_workspace);
    }

    let provisioner_cleanup = match &remote.resources.provisioner {
        Some(provisioner) => failed_workspace_from_result(
            workspace,
            provider
                .terminate_provisioner(TerminateProvisionerParams {
                    workspace_id: workspace.id.clone(),
                    provisioner_id: provisioner.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteProvisionerNotFound,
        ),
        None => None,
    };

    if let Some(failed_workspace) = provisioner_cleanup {
        return Ok(failed_workspace);
    }

    let volume_cleanup = match &remote.resources.volume {
        Some(volume) => failed_workspace_from_result(
            workspace,
            provider
                .delete_volume(DeleteVolumeParams {
                    workspace_id: workspace.id.clone(),
                    volume_id: volume.id.clone(),
                })
                .await,
            ProvisionedRemoteComputeError::RemoteVolumeNotFound,
        ),
        None => None,
    };

    if let Some(failed_workspace) = volume_cleanup {
        return Ok(failed_workspace);
    }

    Ok(reset_remote_state(workspace))
}
