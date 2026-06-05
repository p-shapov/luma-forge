use crate::domain::workspace::{
    RemoteProvisioningError, RemoteProvisioningPhase, RemoteProvisioningStatus, RemoteWorkspace,
    RemoteWorkspaceResources, Workspace, WorkspaceRuntime,
};

use super::errors::RemoteWorkspaceError;

pub fn failed_workspace_from_result(
    workspace: &Workspace,
    result: Result<(), RemoteWorkspaceError>,
    ignored_error: RemoteWorkspaceError,
) -> Option<Workspace> {
    ignore_expected_error(result, ignored_error)
        .err()
        .map(|error| {
            with_provisioning_failure(workspace, None, RemoteProvisioningError::from(error))
        })
}

pub fn ignore_expected_error(
    result: Result<(), RemoteWorkspaceError>,
    ignored_error: RemoteWorkspaceError,
) -> Result<(), RemoteWorkspaceError> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error == ignored_error => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn remote_runtime(workspace: &Workspace) -> Result<&RemoteWorkspace, RemoteWorkspaceError> {
    // When WorkspaceRuntime gets non-remote variants, return an explicit
    // RemoteWorkspaceError here instead of accepting them in this service.
    let WorkspaceRuntime::Remote(remote) = &workspace.runtime;
    Ok(remote)
}

pub fn with_provisioning_failure(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    error: RemoteProvisioningError,
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_provisioning.status = RemoteProvisioningStatus::Failed { phase, error };
    workspace
}

pub fn with_cleanup_failure(
    workspace: &Workspace,
    phase: Option<RemoteProvisioningPhase>,
    error: RemoteWorkspaceError,
) -> Workspace {
    let provisioning_error = match error {
        RemoteWorkspaceError::Provider(error) => RemoteProvisioningError::Provider(error),
        _ => RemoteProvisioningError::CancellationCleanupFailed,
    };
    with_provisioning_failure(workspace, phase, provisioning_error)
}

pub fn reset_remote_state(workspace: &Workspace) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    remote.remote_resources = RemoteWorkspaceResources {
        remote_volume: None,
        remote_provisioner: None,
        remote_endpoint: None,
    };
    remote.remote_provisioning.status = RemoteProvisioningStatus::NotStarted;
    remote.remote_provisioning.percent = None;
    workspace
}

pub fn with_status_and_resources(
    workspace: &Workspace,
    status: RemoteProvisioningStatus,
    percent: u8,
    update_resources: impl FnOnce(&mut RemoteWorkspaceResources),
) -> Workspace {
    let mut workspace = workspace.clone();
    let WorkspaceRuntime::Remote(remote) = &mut workspace.runtime;
    update_resources(&mut remote.remote_resources);
    remote.remote_provisioning.status = status;
    remote.remote_provisioning.percent = Some(percent);
    workspace
}
