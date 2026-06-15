use sqlx::SqlitePool;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::{
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceCatalog},
    },
    shared::AppFuture,
};

use super::{
    errors::{data_invalid_message, storage_unavailable_error, WorkspaceCatalogError},
    repository::WorkspaceCatalogRepository,
    runtime,
};

mod row;
mod state;

use row::workspace_from_row;
use state::workspace_state_columns;

const LIST_WORKSPACES_SQL: &str = "SELECT id, runtime_type, state, workflow_id, \
    workflow_version, runtime_json FROM workspaces ORDER BY created_at ASC";
const FIND_WORKSPACE_SQL: &str = "SELECT id, runtime_type, state, workflow_id, \
    workflow_version, runtime_json FROM workspaces WHERE id = ?1";

struct PersistedWorkspace<'a> {
    workspace: &'a Workspace,
    runtime_type: String,
    runtime_json: String,
    state: &'static str,
}

impl<'a> PersistedWorkspace<'a> {
    fn encode(workspace: &'a Workspace) -> Result<Self, WorkspaceCatalogError> {
        validate_id(&workspace.id)?;
        validate_workflow_reference(&workspace.workflow)?;

        let encoded = runtime::encode_runtime(&workspace.runtime)?;
        let state = workspace_state_columns(&workspace.state);

        Ok(Self {
            workspace,
            runtime_type: encoded.runtime_type,
            runtime_json: encoded.runtime_json,
            state: state.state,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalogRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            let rows = sqlx::query(LIST_WORKSPACES_SQL)
                .fetch_all(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;
            let workspaces = rows
                .iter()
                .map(workspace_from_row)
                .collect::<Result<Vec<_>, _>>()?;

            Ok(WorkspaceCatalog { workspaces })
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>> {
        Box::pin(async move {
            validate_id(id)?;

            let row = sqlx::query(FIND_WORKSPACE_SQL)
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;

            row.as_ref().map(workspace_from_row).transpose()
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            let persisted = PersistedWorkspace::encode(workspace)?;
            let now = timestamp()?;

            sqlx::query(
                "INSERT INTO workspaces (
                    id,
                    runtime_type,
                    state,
                    workflow_id,
                    workflow_version,
                    runtime_json,
                    created_at,
                    updated_at
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )
            .bind(&persisted.workspace.id)
            .bind(&persisted.runtime_type)
            .bind(persisted.state)
            .bind(&persisted.workspace.workflow.id)
            .bind(&persisted.workspace.workflow.version)
            .bind(&persisted.runtime_json)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    WorkspaceCatalogError::WorkspaceAlreadyExists
                } else {
                    storage_unavailable_error(error)
                }
            })?;

            Ok(workspace.clone())
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            let persisted = PersistedWorkspace::encode(workspace)?;
            let now = timestamp()?;

            let result = sqlx::query(
                "UPDATE workspaces
                 SET runtime_type = ?1,
                     state = ?2,
                     workflow_id = ?3,
                     workflow_version = ?4,
                     runtime_json = ?5,
                     updated_at = ?6
                 WHERE id = ?7",
            )
            .bind(&persisted.runtime_type)
            .bind(persisted.state)
            .bind(&persisted.workspace.workflow.id)
            .bind(&persisted.workspace.workflow.version)
            .bind(&persisted.runtime_json)
            .bind(now)
            .bind(&persisted.workspace.id)
            .execute(&self.pool)
            .await
            .map_err(storage_unavailable_error)?;

            if result.rows_affected() == 0 {
                return Err(WorkspaceCatalogError::WorkspaceNotFound);
            }

            Ok(workspace.clone())
        })
    }

    fn delete_workspace<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
        Box::pin(async move {
            validate_id(id)?;

            let result = sqlx::query("DELETE FROM workspaces WHERE id = ?1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;

            if result.rows_affected() == 0 {
                return Err(WorkspaceCatalogError::WorkspaceNotFound);
            }

            Ok(())
        })
    }
}

pub(super) fn validate_id(id: &str) -> Result<(), WorkspaceCatalogError> {
    if id.trim().is_empty() {
        Err(data_invalid_message("ID is empty"))
    } else {
        Ok(())
    }
}

pub(super) fn validate_workflow_reference(
    workflow: &WorkflowReference,
) -> Result<(), WorkspaceCatalogError> {
    validate_required_text(&workflow.id, "workflow ID is missing")?;
    validate_required_text(&workflow.version, "workflow version is missing")
}

fn validate_required_text(value: &str, message: &'static str) -> Result<(), WorkspaceCatalogError> {
    if value.trim().is_empty() {
        Err(data_invalid_message(message))
    } else {
        Ok(())
    }
}

fn timestamp() -> Result<String, WorkspaceCatalogError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(storage_unavailable_error)
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error
            .code()
            .is_some_and(|code| code.as_ref() == "1555" || code.as_ref() == "2067"),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
