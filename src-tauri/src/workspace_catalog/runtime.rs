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
        WorkspaceRuntime::Runpod(runtime) => runtimes::runpod::encode(runtime),
    }
}

pub fn decode_runtime(
    runtime_type: &str,
    runtime_json: &str,
) -> Result<WorkspaceRuntime, WorkspaceCatalogError> {
    match runtime_type {
        runtimes::runpod::RUNTIME_TYPE => runtimes::runpod::decode(runtime_json),
        unknown_runtime_type => Err(WorkspaceCatalogError::DataInvalid {
            message: format!("unknown runtime type: {}", unknown_runtime_type),
        }),
    }
}
