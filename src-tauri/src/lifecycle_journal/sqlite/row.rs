use sqlx::Row;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::lifecycle_operation::{LifecycleOperation, LifecycleOperationState},
    lifecycle_journal::{
        errors::{data_invalid_error, data_invalid_message, schema_invalid_message},
        payload::decode_payload,
        LifecycleJournalError,
    },
};

pub(super) fn operation_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<LifecycleOperation, LifecycleJournalError> {
    let operation_id = required_text(row, "id", "operation ID is missing")?;
    let workspace_id = required_text(row, "workspace_id", "workspace ID is missing")?;
    let state = required_text(row, "state", "state is missing")?;
    let payload_json = required_text(row, "payload_json", "payload JSON is missing")?;
    let created_at = required_text(row, "created_at", "created_at is missing")?;
    let updated_at = required_text(row, "updated_at", "updated_at is missing")?;
    let finished_at = row
        .try_get::<Option<String>, _>("finished_at")
        .map_err(|_| schema_invalid_message("finished_at is missing"))?;

    if operation_id.trim().is_empty() {
        return Err(data_invalid_message("operation ID is empty"));
    }
    if workspace_id.trim().is_empty() {
        return Err(data_invalid_message("workspace ID is empty"));
    }

    Ok(LifecycleOperation {
        operation_id,
        workspace_id,
        state: state_from_storage(&state)?,
        payload: decode_payload(&payload_json)?,
        created_at: parse_timestamp(&created_at)?,
        updated_at: parse_timestamp(&updated_at)?,
        finished_at: finished_at.as_deref().map(parse_timestamp).transpose()?,
    })
}

pub(super) fn state_to_storage(state: LifecycleOperationState) -> &'static str {
    match state {
        LifecycleOperationState::Running => "running",
        LifecycleOperationState::Completed => "completed",
        LifecycleOperationState::Failed => "failed",
        LifecycleOperationState::Stale => "stale",
    }
}

fn required_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
    missing_message: &'static str,
) -> Result<String, LifecycleJournalError> {
    row.try_get(column)
        .map_err(|_| schema_invalid_message(missing_message))
}

fn state_from_storage(state: &str) -> Result<LifecycleOperationState, LifecycleJournalError> {
    match state {
        "running" => Ok(LifecycleOperationState::Running),
        "completed" => Ok(LifecycleOperationState::Completed),
        "failed" => Ok(LifecycleOperationState::Failed),
        "stale" => Ok(LifecycleOperationState::Stale),
        state => Err(data_invalid_message(format!(
            "unknown lifecycle operation state: {state}"
        ))),
    }
}

fn parse_timestamp(timestamp: &str) -> Result<OffsetDateTime, LifecycleJournalError> {
    OffsetDateTime::parse(timestamp, &Rfc3339).map_err(data_invalid_error)
}
