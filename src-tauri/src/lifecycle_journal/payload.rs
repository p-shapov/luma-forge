use crate::domain::lifecycle_operation::LifecycleOperationPayload;

use super::{payloads, LifecycleJournalError};

pub fn encode_payload(
    payload: &LifecycleOperationPayload,
) -> Result<String, LifecycleJournalError> {
    match payload {
        LifecycleOperationPayload::Runpod(payload) => payloads::runpod::encode(payload),
    }
}

pub fn decode_payload(
    payload_json: &str,
) -> Result<LifecycleOperationPayload, LifecycleJournalError> {
    let value: serde_json::Value =
        serde_json::from_str(payload_json).map_err(|_| LifecycleJournalError::Corrupt)?;
    let runtime_type = value
        .get("runtime_type")
        .and_then(serde_json::Value::as_str)
        .ok_or(LifecycleJournalError::Corrupt)?;

    match runtime_type {
        payloads::runpod::RUNTIME_TYPE => payloads::runpod::decode(payload_json),
        _ => Err(LifecycleJournalError::Corrupt),
    }
}
