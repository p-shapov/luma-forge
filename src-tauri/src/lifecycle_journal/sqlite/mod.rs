use sqlx::SqlitePool;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    domain::lifecycle_operation::{
        LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
        LifecycleOperationState,
    },
    domain::workspace::WorkspaceId,
    shared::AppFuture,
};

use super::{
    errors::{data_invalid_message, storage_unavailable_error},
    payload::encode_payload,
    repository::LifecycleJournalRepository,
    LifecycleJournalError,
};

mod row;

use row::{operation_from_row, state_to_storage};

#[derive(Debug, Clone)]
pub struct SqliteLifecycleJournalRepository {
    pool: SqlitePool,
}

impl SqliteLifecycleJournalRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl LifecycleJournalRepository for SqliteLifecycleJournalRepository {
    fn create_operation<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
        payload: &'a LifecycleOperationPayload,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
            validate_workspace_id(workspace_id)?;

            let operation_id = Uuid::new_v4().to_string();
            let payload_json = encode_payload(payload)?;
            let now = timestamp()?;

            sqlx::query(
                "INSERT INTO lifecycle_operations (
                    id,
                    workspace_id,
                    state,
                    payload_json,
                    created_at,
                    updated_at,
                    finished_at
                )
                 VALUES (?1, ?2, 'running', ?3, ?4, ?5, NULL)",
            )
            .bind(&operation_id)
            .bind(workspace_id)
            .bind(payload_json)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    LifecycleJournalError::RunningOperationExists
                } else {
                    storage_unavailable_error(error)
                }
            })?;

            self.find_by_id(&operation_id).await
        })
    }

    fn find_running_by_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
            validate_workspace_id(workspace_id)?;

            let row = sqlx::query(
                "SELECT id, workspace_id, state, payload_json, created_at, updated_at, finished_at
                 FROM lifecycle_operations
                 WHERE workspace_id = ?1 AND state = 'running'
                 LIMIT 1",
            )
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_unavailable_error)?;

            row.as_ref().map(operation_from_row).transpose()
        })
    }

    fn list_running<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<Vec<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, workspace_id, state, payload_json, created_at, updated_at, finished_at
                 FROM lifecycle_operations
                 WHERE state = 'running'
                 ORDER BY created_at ASC, id ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(storage_unavailable_error)?;

            rows.iter().map(operation_from_row).collect()
        })
    }

    fn latest_for_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
            validate_workspace_id(workspace_id)?;

            let row = sqlx::query(
                "SELECT id, workspace_id, state, payload_json, created_at, updated_at, finished_at
                 FROM lifecycle_operations
                 WHERE workspace_id = ?1
                 ORDER BY created_at DESC, updated_at DESC, id DESC
                 LIMIT 1",
            )
            .bind(workspace_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_unavailable_error)?;

            row.as_ref().map(operation_from_row).transpose()
        })
    }

    fn delete_for_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<(), LifecycleJournalError>> {
        Box::pin(async move {
            validate_workspace_id(workspace_id)?;

            sqlx::query("DELETE FROM lifecycle_operations WHERE workspace_id = ?1")
                .bind(workspace_id)
                .execute(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;

            Ok(())
        })
    }

    fn update_operation<'a>(
        &'a self,
        operation: &'a LifecycleOperation,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
            validate_operation_for_update(operation)?;

            let existing = self.find_by_id(&operation.operation_id).await?;
            validate_operation_identity_matches(&existing, operation)?;

            let payload_json = encode_payload(&operation.payload)?;
            let finished_at = finished_at_for_update(
                operation.state,
                operation.updated_at,
                operation.finished_at,
            )?;
            let finished_at_storage = format_optional_timestamp(finished_at)?;

            let result = sqlx::query(
                "UPDATE lifecycle_operations
                 SET state = ?1,
                     payload_json = ?2,
                     updated_at = ?3,
                     finished_at = ?4
                 WHERE id = ?5",
            )
            .bind(state_to_storage(operation.state))
            .bind(payload_json)
            .bind(format_timestamp(operation.updated_at)?)
            .bind(finished_at_storage)
            .bind(&operation.operation_id)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    LifecycleJournalError::RunningOperationExists
                } else {
                    storage_unavailable_error(error)
                }
            })?;

            if result.rows_affected() == 0 {
                return Err(LifecycleJournalError::OperationNotFound);
            }

            self.find_by_id(&operation.operation_id).await
        })
    }

    fn mark_state<'a>(
        &'a self,
        operation_id: &'a LifecycleOperationId,
        state: LifecycleOperationState,
        payload: &'a LifecycleOperationPayload,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
            validate_operation_id(operation_id)?;

            let current = self.find_by_id(operation_id).await?;
            if current.state != LifecycleOperationState::Running {
                return Err(LifecycleJournalError::OperationNotFound);
            }

            let payload_json = encode_payload(payload)?;
            let updated_at = timestamp()?;
            let finished_at = if state == LifecycleOperationState::Running {
                None
            } else {
                Some(updated_at.clone())
            };

            let result = sqlx::query(
                "UPDATE lifecycle_operations
                 SET state = ?1,
                     payload_json = ?2,
                     updated_at = ?3,
                     finished_at = ?4
                 WHERE id = ?5 AND state = 'running'",
            )
            .bind(state_to_storage(state))
            .bind(payload_json)
            .bind(&updated_at)
            .bind(finished_at)
            .bind(operation_id)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    LifecycleJournalError::RunningOperationExists
                } else {
                    storage_unavailable_error(error)
                }
            })?;

            if result.rows_affected() == 0 {
                return Err(LifecycleJournalError::OperationNotFound);
            }

            self.find_by_id(operation_id).await
        })
    }
}

impl SqliteLifecycleJournalRepository {
    async fn find_by_id(
        &self,
        operation_id: &str,
    ) -> Result<LifecycleOperation, LifecycleJournalError> {
        let row = sqlx::query(
            "SELECT id, workspace_id, state, payload_json, created_at, updated_at, finished_at
             FROM lifecycle_operations
             WHERE id = ?1",
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage_unavailable_error)?;

        row.as_ref()
            .map(operation_from_row)
            .transpose()?
            .ok_or(LifecycleJournalError::OperationNotFound)
    }
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), LifecycleJournalError> {
    if workspace_id.trim().is_empty() {
        Err(data_invalid_message("workspace ID is empty"))
    } else {
        Ok(())
    }
}

fn validate_operation_id(operation_id: &str) -> Result<(), LifecycleJournalError> {
    if operation_id.trim().is_empty() {
        Err(LifecycleJournalError::OperationNotFound)
    } else {
        Ok(())
    }
}

fn validate_operation_for_update(
    operation: &LifecycleOperation,
) -> Result<(), LifecycleJournalError> {
    validate_operation_id(&operation.operation_id)?;
    validate_workspace_id(&operation.workspace_id)
}

fn validate_operation_identity_matches(
    existing: &LifecycleOperation,
    supplied: &LifecycleOperation,
) -> Result<(), LifecycleJournalError> {
    if existing.workspace_id != supplied.workspace_id || existing.created_at != supplied.created_at
    {
        Err(data_invalid_message(
            "operation identity fields do not match persisted operation",
        ))
    } else {
        Ok(())
    }
}

fn timestamp() -> Result<String, LifecycleJournalError> {
    format_timestamp(OffsetDateTime::now_utc())
}

fn format_timestamp(timestamp: OffsetDateTime) -> Result<String, LifecycleJournalError> {
    timestamp
        .format(&Rfc3339)
        .map_err(storage_unavailable_error)
}

fn format_optional_timestamp(
    timestamp: Option<OffsetDateTime>,
) -> Result<Option<String>, LifecycleJournalError> {
    timestamp.map(format_timestamp).transpose()
}

fn finished_at_for_update(
    state: LifecycleOperationState,
    updated_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
) -> Result<Option<OffsetDateTime>, LifecycleJournalError> {
    if state == LifecycleOperationState::Running {
        Ok(None)
    } else if let Some(finished_at) = finished_at {
        if finished_at < updated_at {
            Err(data_invalid_message("finished_at is before updated_at"))
        } else {
            Ok(Some(finished_at))
        }
    } else {
        Ok(Some(updated_at))
    }
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
