use crate::domain::workspace::WorkspaceRuntime;

use super::{
    contracts,
    errors::{data_invalid_message, WorkspaceCatalogError},
};

pub struct EncodedWorkspaceRuntime {
    pub runtime_type: String,
    pub runtime_json: String,
}

pub fn encode_runtime(
    runtime: &WorkspaceRuntime,
) -> Result<EncodedWorkspaceRuntime, WorkspaceCatalogError> {
    match runtime {
        WorkspaceRuntime::Runpod(runtime) => contracts::runpod::encode(runtime),
    }
}

pub fn decode_runtime(
    runtime_type: &str,
    runtime_json: &str,
) -> Result<WorkspaceRuntime, WorkspaceCatalogError> {
    match runtime_type {
        contracts::runpod::RUNTIME_TYPE => contracts::runpod::decode(runtime_json),
        unknown_runtime_type => Err(data_invalid_message(format!(
            "unknown runtime type: {unknown_runtime_type}"
        ))),
    }
}
