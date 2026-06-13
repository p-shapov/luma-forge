use crate::domain::{
    lifecycle_operation::LifecycleOperationPayload, runpod::RunpodLifecycleOperationPayload,
};

use super::super::{LifecycleJournalError, errors::data_invalid_error};

pub const RUNTIME_TYPE: &str = "runpod";

pub fn encode(payload: &RunpodLifecycleOperationPayload) -> Result<String, LifecycleJournalError> {
    serde_json::to_string(&LifecycleOperationPayload::Runpod(payload.clone()))
        .map_err(data_invalid_error)
}

pub fn decode(payload_json: &str) -> Result<LifecycleOperationPayload, LifecycleJournalError> {
    let payload: LifecycleOperationPayload =
        serde_json::from_str(payload_json).map_err(data_invalid_error)?;

    match payload {
        LifecycleOperationPayload::Runpod(_) => Ok(payload),
    }
}
