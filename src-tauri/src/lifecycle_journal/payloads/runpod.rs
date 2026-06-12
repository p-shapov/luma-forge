use crate::domain::lifecycle_operation::{
    LifecycleOperationPayload, RunpodLifecycleOperationPayload,
};

use super::super::LifecycleJournalError;

pub const RUNTIME_TYPE: &str = "runpod";

pub fn encode(payload: &RunpodLifecycleOperationPayload) -> Result<String, LifecycleJournalError> {
    serde_json::to_string(&LifecycleOperationPayload::Runpod(payload.clone()))
        .map_err(|_| LifecycleJournalError::Corrupt)
}

pub fn decode(payload_json: &str) -> Result<LifecycleOperationPayload, LifecycleJournalError> {
    let payload: LifecycleOperationPayload =
        serde_json::from_str(payload_json).map_err(|_| LifecycleJournalError::Corrupt)?;

    match payload {
        LifecycleOperationPayload::Runpod(_) => Ok(payload),
    }
}
