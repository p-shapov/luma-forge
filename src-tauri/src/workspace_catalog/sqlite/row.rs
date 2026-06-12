use sqlx::Row;

use crate::domain::{workflow_preset::WorkflowReference, workspace::Workspace};

use super::{state::workspace_state_from_columns, validate_id, validate_workflow_reference};
use crate::workspace_catalog::{errors::WorkspaceCatalogError, runtime};

pub(super) fn workspace_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<Workspace, WorkspaceCatalogError> {
    let id = required_text(row, "id", "ID is missing")?;
    let runtime_type = required_text(row, "runtime_type", "runtime type is missing")?;
    let state = required_text(row, "state", "state is missing")?;
    let state_reason = optional_text(row, "state_reason", "state reason is missing")?;
    let workflow_id = required_text(row, "workflow_id", "workflow ID is missing")?;
    let workflow_version = required_text(row, "workflow_version", "workflow version is missing")?;
    let runtime_json = required_text(row, "runtime_json", "runtime JSON is missing")?;
    validate_id(&id)?;

    let workflow = WorkflowReference {
        id: workflow_id,
        version: workflow_version,
    };
    validate_workflow_reference(&workflow)?;

    Ok(Workspace {
        id,
        workflow,
        state: workspace_state_from_columns(&state, state_reason.as_deref())?,
        runtime: runtime::decode_runtime(&runtime_type, &runtime_json)?,
    })
}

fn required_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
    missing_message: &'static str,
) -> Result<String, WorkspaceCatalogError> {
    row.try_get(column)
        .map_err(|_| schema_invalid(missing_message))
}

fn optional_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
    missing_message: &'static str,
) -> Result<Option<String>, WorkspaceCatalogError> {
    row.try_get(column)
        .map_err(|_| schema_invalid(missing_message))
}

fn schema_invalid(message: &'static str) -> WorkspaceCatalogError {
    WorkspaceCatalogError::SchemaInvalid {
        message: message.to_string(),
    }
}
