use crate::domain::{runpod::runtime::RunpodRuntime, workspace::WorkspaceRuntime};

use super::super::{errors::WorkspaceCatalogError, runtime::EncodedWorkspaceRuntime};

pub const RUNTIME_TYPE: &str = "runpod";

pub fn encode(runtime: &RunpodRuntime) -> Result<EncodedWorkspaceRuntime, WorkspaceCatalogError> {
    Ok(EncodedWorkspaceRuntime {
        runtime_type: RUNTIME_TYPE.to_string(),
        runtime_json: serde_json::to_string(runtime).map_err(|error| {
            WorkspaceCatalogError::DataInvalid {
                message: format!("runpod runtime serialization failed: {error}"),
            }
        })?,
    })
}

pub fn decode(runtime_json: &str) -> Result<WorkspaceRuntime, WorkspaceCatalogError> {
    let runtime: RunpodRuntime =
        serde_json::from_str(runtime_json).map_err(|error| WorkspaceCatalogError::DataInvalid {
            message: format!("runpod runtime JSON is invalid: {error}"),
        })?;

    Ok(WorkspaceRuntime::Runpod(runtime))
}
