use crate::domain::workspace::{
    ProvisionedRemoteComputeProvisionerStatus, ProvisionedRemoteComputeProvisioningError,
    ProvisionedRemoteComputeProvisioningPhase, ProvisionedRemoteComputeProvisioningStatus,
    ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
};
use crate::workflow_catalog::WorkflowCatalogService;

use super::{
    contracts::ProvisionedRemoteComputeContractResolver,
    errors::ProvisionedRemoteComputeError,
    helpers::{with_provisioning_failure, with_status_and_resources},
    provider::{
        CreateEndpointParams, CreateVolumeParams, GetProvisionerStatusParams,
        ProvisionedRemoteComputeProvider, StartProvisionerParams, TerminateProvisionerParams,
    },
};

pub(crate) struct ProvisionedRemoteComputeFlowContext<'a> {
    pub(crate) workflow_catalog_service: &'a WorkflowCatalogService,
    pub(crate) provider: &'a dyn ProvisionedRemoteComputeProvider,
}

pub(crate) fn handle_terminal_status(
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    Ok(workspace.clone())
}

pub(crate) fn handle_not_started(
    workspace: &Workspace,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    Ok(with_status_and_resources(
        workspace,
        ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume,
        },
        0,
        |_| {},
    ))
}

pub(crate) async fn handle_creating_volume(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let volume = match context
        .provider
        .create_volume(CreateVolumeParams {
            workspace_id: workspace.id.clone(),
            datacenter_id: remote.remote_placement.datacenter_id.clone(),
            gpu_id: remote.remote_placement.gpu_id.clone(),
            size_bytes: remote.remote_placement.volume_size_bytes,
            mount_path: "/workspace".to_string(),
        })
        .await
    {
        Ok(volume) => volume,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteVolume),
                error.into(),
            ));
        }
    };

    Ok(with_status_and_resources(
        workspace,
        ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::StartingRemoteProvisioner,
        },
        25,
        |resources| {
            resources.volume = Some(volume);
        },
    ))
}

pub(crate) async fn handle_starting_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
    phase: &ProvisionedRemoteComputeProvisioningPhase,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let volume = match remote.resources.volume.as_ref() {
        Some(volume) => volume,
        None => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote volume snapshot is required before provisioner start"
                        .to_string(),
                },
            ));
        }
    };
    let resolver = ProvisionedRemoteComputeContractResolver::new(context.workflow_catalog_service);
    let provisioner_image_ref = match resolver.provisioner_image_ref(workspace, remote) {
        Ok(image_ref) => image_ref,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error,
            ));
        }
    };

    let provisioner = match context
        .provider
        .start_provisioner(StartProvisionerParams {
            workspace_id: workspace.id.clone(),
            datacenter_id: remote.remote_placement.datacenter_id.clone(),
            gpu_id: remote.remote_placement.gpu_id.clone(),
            volume_id: volume.id.clone(),
            provisioner_image_ref,
            mount_path: "/workspace".to_string(),
            requires_hugging_face_api_key: workspace.workflow_preset.requires_hugging_face_api_key,
        })
        .await
    {
        Ok(provisioner) => provisioner,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error.into(),
            ));
        }
    };

    Ok(with_status_and_resources(
        workspace,
        ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                status: ProvisionedRemoteComputeProvisionerStatus::Pending,
            },
        },
        50,
        |resources| {
            resources.provisioner = Some(provisioner);
        },
    ))
}

pub(crate) async fn handle_cleaning_up_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
    phase: &ProvisionedRemoteComputeProvisioningPhase,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let provisioner = match remote.resources.provisioner.as_ref() {
        Some(provisioner) => provisioner,
        None => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote provisioner snapshot is required before provisioner cleanup"
                        .to_string(),
                },
            ));
        }
    };
    let status = match context
        .provider
        .get_provisioner_status(GetProvisionerStatusParams {
            workspace_id: workspace.id.clone(),
            provisioner_id: provisioner.id.clone(),
            status_url: provisioner.status_url.clone(),
        })
        .await
    {
        Ok(status) => status,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error.into(),
            ));
        }
    };

    if !matches!(
        status,
        ProvisionedRemoteComputeProvisionerStatus::Succeeded
            | ProvisionedRemoteComputeProvisionerStatus::Failed { .. }
    ) {
        return Ok(with_provisioning_failure(
            workspace,
            Some(phase.clone()),
            ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                message: format!("cleanup requires finished provisioner status: {status:?}"),
            },
        ));
    }

    let termination_result = context
        .provider
        .terminate_provisioner(TerminateProvisionerParams {
            workspace_id: workspace.id.clone(),
            provisioner_id: provisioner.id.clone(),
        })
        .await;

    match (status, termination_result) {
        (ProvisionedRemoteComputeProvisionerStatus::Succeeded, Ok(())) => {
            Ok(with_status_and_resources(
                workspace,
                ProvisionedRemoteComputeProvisioningStatus::InProgress {
                    phase: ProvisionedRemoteComputeProvisioningPhase::CreatingRemoteEndpoint,
                },
                75,
                |resources| {
                    resources.provisioner = None;
                },
            ))
        }
        (ProvisionedRemoteComputeProvisionerStatus::Succeeded, Err(error)) => {
            Ok(with_provisioning_failure(
                workspace,
                Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                    },
                ),
                error.into(),
            ))
        }
        (ProvisionedRemoteComputeProvisionerStatus::Failed { code, message }, Ok(())) => {
            let mut workspace = with_provisioning_failure(
                workspace,
                Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Failed { code, message },
                    },
                ),
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
            );
            let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
            remote.resources.provisioner = None;
            Ok(workspace)
        }
        (ProvisionedRemoteComputeProvisionerStatus::Failed { code, message }, Err(_)) => {
            Ok(with_provisioning_failure(
                workspace,
                Some(
                    ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                        status: ProvisionedRemoteComputeProvisionerStatus::Failed { code, message },
                    },
                ),
                ProvisionedRemoteComputeProvisioningError::ProvisionerWorkerFailed,
            ))
        }
        _ => unreachable!("cleanup-ready status was validated before termination"),
    }
}

pub(crate) async fn handle_running_provisioner(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
    phase: &ProvisionedRemoteComputeProvisioningPhase,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let provisioner = match remote.resources.provisioner.as_ref() {
        Some(provisioner) => provisioner,
        None => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote provisioner snapshot is required before status polling"
                        .to_string(),
                },
            ));
        }
    };
    let status = match context
        .provider
        .get_provisioner_status(GetProvisionerStatusParams {
            workspace_id: workspace.id.clone(),
            provisioner_id: provisioner.id.clone(),
            status_url: provisioner.status_url.clone(),
        })
        .await
    {
        Ok(status) => status,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error.into(),
            ));
        }
    };

    let percent = match &status {
        ProvisionedRemoteComputeProvisionerStatus::Pending
        | ProvisionedRemoteComputeProvisionerStatus::Starting => 50,
        ProvisionedRemoteComputeProvisionerStatus::Running => 60,
        ProvisionedRemoteComputeProvisionerStatus::Succeeded
        | ProvisionedRemoteComputeProvisionerStatus::Failed { .. } => 75,
        ProvisionedRemoteComputeProvisionerStatus::CleaningUp => 75,
    };
    let provisioning_status = match status {
        ProvisionedRemoteComputeProvisionerStatus::Succeeded
        | ProvisionedRemoteComputeProvisionerStatus::Failed { .. } => {
            ProvisionedRemoteComputeProvisioningStatus::InProgress {
                phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner {
                    status: ProvisionedRemoteComputeProvisionerStatus::CleaningUp,
                },
            }
        }
        status => ProvisionedRemoteComputeProvisioningStatus::InProgress {
            phase: ProvisionedRemoteComputeProvisioningPhase::RunningRemoteProvisioner { status },
        },
    };

    Ok(with_status_and_resources(
        workspace,
        provisioning_status,
        percent,
        |_| {},
    ))
}

pub(crate) async fn handle_creating_endpoint(
    workspace: &Workspace,
    remote: &ProvisionedRemoteComputeWorkspace,
    context: &ProvisionedRemoteComputeFlowContext<'_>,
    phase: &ProvisionedRemoteComputeProvisioningPhase,
) -> Result<Workspace, ProvisionedRemoteComputeError> {
    let volume = match remote.resources.volume.as_ref() {
        Some(volume) => volume,
        None => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                ProvisionedRemoteComputeProvisioningError::InvalidProvisioningState {
                    message: "remote volume snapshot is required before endpoint creation"
                        .to_string(),
                },
            ));
        }
    };
    let resolver = ProvisionedRemoteComputeContractResolver::new(context.workflow_catalog_service);
    let endpoint_image_ref = match resolver.endpoint_image_ref(workspace, remote) {
        Ok(image_ref) => image_ref,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error,
            ));
        }
    };
    let endpoint = match context
        .provider
        .create_endpoint(CreateEndpointParams {
            workspace_id: workspace.id.clone(),
            datacenter_id: remote.remote_placement.datacenter_id.clone(),
            gpu_id: remote.remote_placement.gpu_id.clone(),
            volume_id: volume.id.clone(),
            endpoint_image_ref,
            mount_path: "/workspace".to_string(),
            keep_alive_limits: remote.remote_placement.keep_alive_limits.clone(),
        })
        .await
    {
        Ok(endpoint) => endpoint,
        Err(error) => {
            return Ok(with_provisioning_failure(
                workspace,
                Some(phase.clone()),
                error.into(),
            ));
        }
    };

    Ok(with_status_and_resources(
        workspace,
        ProvisionedRemoteComputeProvisioningStatus::Completed,
        100,
        |resources| {
            resources.endpoint = Some(endpoint);
        },
    ))
}
