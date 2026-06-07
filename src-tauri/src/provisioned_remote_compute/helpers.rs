use crate::domain::workspace::{
    ProvisionedRemoteComputeProvisioningError, ProvisionedRemoteComputeProvisioningPhase,
    ProvisionedRemoteComputeProvisioningStatus, ProvisionedRemoteComputeResources,
    ProvisionedRemoteComputeWorkspace, Workspace, WorkspaceRuntime,
};

use super::errors::ProvisionedRemoteComputeError;

pub fn failed_workspace_from_result(
    workspace: &Workspace,
    result: Result<(), ProvisionedRemoteComputeError>,
    ignored_error: ProvisionedRemoteComputeError,
) -> Option<Workspace> {
    ignore_expected_error(result, ignored_error)
        .err()
        .map(|error| {
            with_provisioning_failure(
                workspace,
                None,
                ProvisionedRemoteComputeProvisioningError::from(error),
            )
        })
}

pub fn ignore_expected_error(
    result: Result<(), ProvisionedRemoteComputeError>,
    ignored_error: ProvisionedRemoteComputeError,
) -> Result<(), ProvisionedRemoteComputeError> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error == ignored_error => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn remote_runtime(
    workspace: &Workspace,
) -> Result<&ProvisionedRemoteComputeWorkspace, ProvisionedRemoteComputeError> {
    // When WorkspaceRuntime gets non-remote variants, return an explicit
    // ProvisionedRemoteComputeError here instead of accepting them in this service.
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &workspace.runtime;
    Ok(remote)
}

pub fn with_provisioning_failure(
    workspace: &Workspace,
    phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
    error: ProvisionedRemoteComputeProvisioningError,
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
    remote.provisioning.status =
        ProvisionedRemoteComputeProvisioningStatus::Failed { phase, error };
    workspace
}

pub fn with_cleanup_failure(
    workspace: &Workspace,
    phase: Option<ProvisionedRemoteComputeProvisioningPhase>,
    error: ProvisionedRemoteComputeError,
) -> Workspace {
    let provisioning_error = match error {
        ProvisionedRemoteComputeError::Provider(error) => {
            ProvisionedRemoteComputeProvisioningError::Provider(error)
        }
        _ => ProvisionedRemoteComputeProvisioningError::CancellationCleanupFailed,
    };
    with_provisioning_failure(workspace, phase, provisioning_error)
}

pub fn reset_remote_state(workspace: &Workspace) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
    remote.resources = ProvisionedRemoteComputeResources {
        volume: None,
        provisioner: None,
        endpoint: None,
    };
    remote.provisioning.status = ProvisionedRemoteComputeProvisioningStatus::NotStarted;
    remote.provisioning.percent = None;
    workspace
}

pub fn with_status_and_resources(
    workspace: &Workspace,
    status: ProvisionedRemoteComputeProvisioningStatus,
    percent: u8,
    update_resources: impl FnOnce(&mut ProvisionedRemoteComputeResources),
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::ProvisionedRemoteCompute(remote) = &mut workspace.runtime;
    update_resources(&mut remote.resources);
    remote.provisioning.status = status;
    remote.provisioning.percent = Some(percent);
    workspace
}
