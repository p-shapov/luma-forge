use crate::domain::{provisioned_remote::ProvisionedRemoteRuntime, workspace::WorkspaceRuntime};

use super::super::{errors::WorkspaceCatalogError, runtime::EncodedWorkspaceRuntime};

pub const RUNTIME_TYPE: &str = "provisioned_remote";

pub fn encode(
    remote: &ProvisionedRemoteRuntime,
) -> Result<EncodedWorkspaceRuntime, WorkspaceCatalogError> {
    Ok(EncodedWorkspaceRuntime {
        runtime_type: RUNTIME_TYPE.to_string(),
        provider_id: remote.provider_id(),
        runtime_json: serde_json::to_string(remote).map_err(|_| WorkspaceCatalogError::Corrupt)?,
    })
}

pub fn decode(runtime_json: &str) -> Result<WorkspaceRuntime, WorkspaceCatalogError> {
    let remote: ProvisionedRemoteRuntime =
        serde_json::from_str(runtime_json).map_err(|_| WorkspaceCatalogError::Corrupt)?;

    Ok(WorkspaceRuntime::ProvisionedRemote(remote))
}
