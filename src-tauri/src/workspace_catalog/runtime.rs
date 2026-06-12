use crate::domain::workspace::WorkspaceRuntime;

use super::{errors::WorkspaceCatalogError, runtimes};

pub struct EncodedWorkspaceRuntime {
    pub runtime_type: String,
    pub runtime_json: String,
}

pub fn encode_runtime(
    runtime: &WorkspaceRuntime,
) -> Result<EncodedWorkspaceRuntime, WorkspaceCatalogError> {
    match runtime {
        WorkspaceRuntime::Runpod(runtime) => runtimes::provisioned_remote::encode(runtime),
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
