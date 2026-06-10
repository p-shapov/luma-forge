use crate::domain::{provider::GpuCloudProviderId, workspace::WorkspaceRuntime};

use super::{errors::WorkspaceCatalogError, runtimes};

pub struct EncodedWorkspaceRuntime {
    pub runtime_type: String,
    pub provider_id: GpuCloudProviderId,
    pub runtime_json: String,
}

pub fn encode_runtime(
    runtime: &WorkspaceRuntime,
) -> Result<EncodedWorkspaceRuntime, WorkspaceCatalogError> {
    match runtime {
        WorkspaceRuntime::ProvisionedRemote(remote) => runtimes::provisioned_remote::encode(remote),
    }
}

pub fn decode_runtime(
    runtime_type: &str,
    runtime_json: &str,
) -> Result<WorkspaceRuntime, WorkspaceCatalogError> {
    match runtime_type {
        runtimes::provisioned_remote::RUNTIME_TYPE => {
            runtimes::provisioned_remote::decode(runtime_json)
        }
        _ => Err(WorkspaceCatalogError::Corrupt),
    }
}
