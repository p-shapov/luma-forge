use sqlx::Row;

use crate::domain::{workflow_preset::WorkflowReference, workspace::Workspace};

use super::{state::workspace_state_from_columns, validate_id, validate_workflow_reference};
use crate::workspace_catalog::{
    errors::{schema_invalid_error, WorkspaceCatalogError},
    runtime,
};

pub(super) fn workspace_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<Workspace, WorkspaceCatalogError> {
    let id = required_text(row, "id")?;
    let runtime_type = required_text(row, "runtime_type")?;
    let state = required_text(row, "state")?;
    let workflow_id = required_text(row, "workflow_id")?;
    let workflow_version = required_text(row, "workflow_version")?;
    let runtime_json = required_text(row, "runtime_json")?;
    validate_id(&id)?;

    let workflow = WorkflowReference {
        id: workflow_id,
        version: workflow_version,
    };
    validate_workflow_reference(&workflow)?;

    Ok(Workspace {
        id,
        workflow,
        state: workspace_state_from_columns(&state)?,
        runtime: runtime::decode_runtime(&runtime_type, &runtime_json)?,
    })
}

fn required_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<String, WorkspaceCatalogError> {
    row.try_get(column).map_err(schema_invalid_error)
}
