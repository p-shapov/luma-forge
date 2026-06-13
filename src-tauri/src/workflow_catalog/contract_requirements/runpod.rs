use serde_json::Value;

use crate::domain::{
    runpod::RunpodContractRequirements, workflow_preset::WorkflowContractRequirements,
};

#[cfg(test)]
use super::with_runtime_type;
use super::{object_from_value, parse_failed_message, without_runtime_type};
use crate::workflow_catalog::WorkflowCatalogError;

pub const RUNTIME_TYPE: &str = "runpod";

#[cfg(test)]
pub fn encode(requirements: &RunpodContractRequirements) -> Result<Value, WorkflowCatalogError> {
    let value = serde_json::to_value(requirements).map_err(parse_failed)?;
    with_runtime_type(RUNTIME_TYPE, value)
}

pub fn decode(value: Value) -> Result<WorkflowContractRequirements, WorkflowCatalogError> {
    let value = Value::Object(object_from_value(without_runtime_type(value)?)?);
    let requirements: RunpodContractRequirements =
        serde_json::from_value(value).map_err(parse_failed)?;

    Ok(WorkflowContractRequirements::Runpod(requirements))
}

fn parse_failed(error: serde_json::Error) -> WorkflowCatalogError {
    parse_failed_message(error.to_string())
}
