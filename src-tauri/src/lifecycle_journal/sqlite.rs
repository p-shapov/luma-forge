use sqlx::{Executor, Row, SqlitePool};
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
    errors::{
        data_invalid_error, data_invalid_message, schema_invalid_message, storage_unavailable_error,
    },
    repository::LifecycleJournalRepository,
    LifecycleJournalError,
};

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), LifecycleJournalError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS lifecycle_operations (
            id TEXT PRIMARY KEY NOT NULL,
            workspace_id TEXT NOT NULL,
            state TEXT NOT NULL,
            payload_json TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            finished_at TEXT NULL
        )",
    )
    .await
    .map_err(storage_unavailable_error)?;

    pool.execute(
        "CREATE INDEX IF NOT EXISTS idx_lifecycle_operations_workspace_id
         ON lifecycle_operations(workspace_id)",
    )
    .await
    .map_err(storage_unavailable_error)?;

    pool.execute(
        "CREATE INDEX IF NOT EXISTS idx_lifecycle_operations_state
         ON lifecycle_operations(state)",
    )
    .await
    .map_err(storage_unavailable_error)?;

    pool.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_lifecycle_operations_running_workspace_unique
         ON lifecycle_operations(workspace_id)
         WHERE state = 'running'",
    )
    .await
    .map_err(storage_unavailable_error)?;

    Ok(())
}

fn encode_payload(
    payload: Option<&LifecycleOperationPayload>,
) -> Result<Option<String>, LifecycleJournalError> {
    payload
        .map(serde_json::to_string)
        .transpose()
        .map_err(data_invalid_error)
}

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
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
            validate_workspace_id(workspace_id)?;

            let operation_id = Uuid::new_v4().to_string();
            let payload_json = encode_payload(None)?;
            let now = timestamp()?;

            let result = sqlx::query(
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
            .await;

            if let Err(error) = result {
                if is_unique_constraint(&error) {
                    return Err(running_operation_exists_error(self, workspace_id).await?);
                }
                return Err(storage_unavailable_error(error));
            }

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

            let payload_json = encode_payload(operation.payload.as_ref())?;
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
            .await;

            let result = match result {
                Ok(result) => result,
                Err(error) if is_unique_constraint(&error) => {
                    return Err(
                        running_operation_exists_error(self, &operation.workspace_id).await?,
                    );
                }
                Err(error) => return Err(storage_unavailable_error(error)),
            };

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
        payload: Option<&'a LifecycleOperationPayload>,
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
            .await;

            let result = match result {
                Ok(result) => result,
                Err(error) if is_unique_constraint(&error) => {
                    return Err(running_operation_exists_error(self, &current.workspace_id).await?);
                }
                Err(error) => return Err(storage_unavailable_error(error)),
            };

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

fn operation_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<LifecycleOperation, LifecycleJournalError> {
    let operation_id = required_text(row, "id", "operation ID is missing")?;
    let workspace_id = required_text(row, "workspace_id", "workspace ID is missing")?;
    let state = required_text(row, "state", "state is missing")?;
    let payload_json = row
        .try_get::<Option<String>, _>("payload_json")
        .map_err(data_invalid_error)?;
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
        payload: payload_json
            .map(|payload_json| serde_json::from_str::<LifecycleOperationPayload>(&payload_json))
            .transpose()
            .map_err(data_invalid_error)?,
        created_at: parse_timestamp(&created_at)?,
        updated_at: parse_timestamp(&updated_at)?,
        finished_at: finished_at.as_deref().map(parse_timestamp).transpose()?,
    })
}

fn state_to_storage(state: LifecycleOperationState) -> &'static str {
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

async fn running_operation_exists_error(
    repository: &SqliteLifecycleJournalRepository,
    workspace_id: &WorkspaceId,
) -> Result<LifecycleJournalError, LifecycleJournalError> {
    let operation = repository
        .find_running_by_workspace(workspace_id)
        .await?
        .ok_or_else(|| data_invalid_message("running operation unique constraint had no row"))?;
    Ok(LifecycleJournalError::RunningOperationExists {
        operation_id: operation.operation_id,
    })
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
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::sqlite::SqliteConnectOptions;

    use crate::domain::{
        lifecycle_operation::{
            LifecycleCleanupPayload, LifecycleOperationPayload, LifecycleOperationState,
            LifecycleProvisionPayload,
        },
        runpod::{
            RunpodLifecycleCleanupPayload, RunpodLifecycleProvisionPayload, RunpodProvisionStep,
        },
    };

    use super::*;

    fn journal_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-lifecycle-{name}-{nonce}.sqlite"))
    }

    async fn repository(name: &str) -> SqliteLifecycleJournalRepository {
        let options = SqliteConnectOptions::new()
            .filename(journal_path(name))
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("journal db connection should succeed");
        bootstrap(&pool)
            .await
            .expect("journal schema should bootstrap");

        SqliteLifecycleJournalRepository::new(pool)
    }

    fn provision_payload(step: Option<RunpodProvisionStep>) -> LifecycleOperationPayload {
        LifecycleOperationPayload::Provision(LifecycleProvisionPayload::Runpod(
            RunpodLifecycleProvisionPayload { step },
        ))
    }

    #[tokio::test]
    async fn creating_second_running_operation_for_workspace_returns_running_operation_exists() {
        let repository = repository("duplicate-running").await;

        let operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("first operation should be created");
        let error = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect_err("second running operation should fail");

        assert_eq!(
            error,
            LifecycleJournalError::RunningOperationExists {
                operation_id: operation.operation_id,
            }
        );
    }

    #[tokio::test]
    async fn completed_operation_allows_new_running_operation_for_same_workspace() {
        let repository = repository("completed-allows-new").await;
        let operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");

        repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Completed,
                None,
            )
            .await
            .expect("operation should complete");
        assert_eq!(operation.payload, None);
        let next_operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("new running operation should be created");

        assert_ne!(operation.operation_id, next_operation.operation_id);
        assert_eq!(next_operation.state, LifecycleOperationState::Running);
    }

    #[tokio::test]
    async fn delete_for_workspace_removes_only_matching_rows() {
        let repository = repository("delete-for-workspace").await;
        let workspace_1 = "workspace-1".to_string();
        let workspace_2 = "workspace-2".to_string();
        repository
            .create_operation(&workspace_1)
            .await
            .expect("first operation should be created");
        let remaining = repository
            .create_operation(&workspace_2)
            .await
            .expect("second operation should be created");

        repository
            .delete_for_workspace(&workspace_1)
            .await
            .expect("workspace operations should delete");

        assert_eq!(
            repository
                .latest_for_workspace(&workspace_1)
                .await
                .expect("latest should load"),
            None
        );
        assert_eq!(
            repository
                .latest_for_workspace(&workspace_2)
                .await
                .expect("latest should load")
                .expect("remaining operation should exist")
                .operation_id,
            remaining.operation_id
        );
    }

    #[tokio::test]
    async fn mark_state_sets_state_payload_and_finished_at_for_terminal_states() {
        let repository = repository("mark-state").await;
        let finished_payload = provision_payload(Some(RunpodProvisionStep::CreateNetworkVolume));
        let operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");

        let marked = repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Failed,
                Some(&finished_payload),
            )
            .await
            .expect("operation should be marked failed");

        assert_eq!(marked.state, LifecycleOperationState::Failed);
        assert_eq!(marked.payload, Some(finished_payload));
        assert!(marked.finished_at.is_some());
    }

    #[tokio::test]
    async fn update_operation_cannot_change_workspace_id_or_created_at() {
        let repository = repository("update-operation-immutable-fields").await;
        let operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");

        let mut changed_workspace = operation.clone();
        changed_workspace.workspace_id = "workspace-2".to_string();
        let workspace_error = repository
            .update_operation(&changed_workspace)
            .await
            .expect_err("workspace_id change should be corrupt");

        let mut changed_created_at = operation.clone();
        changed_created_at.created_at =
            OffsetDateTime::from_unix_timestamp(42).expect("valid timestamp");
        let created_at_error = repository
            .update_operation(&changed_created_at)
            .await
            .expect_err("created_at change should be corrupt");

        assert!(matches!(
            workspace_error,
            LifecycleJournalError::DataInvalid { .. }
        ));
        assert!(matches!(
            created_at_error,
            LifecycleJournalError::DataInvalid { .. }
        ));
    }

    #[tokio::test]
    async fn terminal_update_with_no_finished_at_uses_operation_updated_at() {
        let repository = repository("terminal-update-finished-at").await;
        let mut operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");
        let updated_at = OffsetDateTime::from_unix_timestamp(42).expect("valid timestamp");
        operation.state = LifecycleOperationState::Completed;
        operation.updated_at = updated_at;
        operation.finished_at = None;

        let updated = repository
            .update_operation(&operation)
            .await
            .expect("operation should be updated");

        assert_eq!(updated.updated_at, updated_at);
        assert_eq!(updated.finished_at, Some(updated_at));
    }

    #[tokio::test]
    async fn terminal_update_rejects_finished_at_before_updated_at() {
        let repository = repository("terminal-update-invalid-finished-at").await;
        let mut operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");
        operation.state = LifecycleOperationState::Failed;
        operation.updated_at = OffsetDateTime::from_unix_timestamp(42).expect("valid timestamp");
        operation.finished_at =
            Some(OffsetDateTime::from_unix_timestamp(41).expect("valid timestamp"));

        let error = repository
            .update_operation(&operation)
            .await
            .expect_err("finished_at before updated_at should be corrupt");

        assert!(matches!(error, LifecycleJournalError::DataInvalid { .. }));
    }

    #[tokio::test]
    async fn mark_state_cannot_reopen_terminal_operation() {
        let repository = repository("mark-state-terminal").await;
        let operation = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation should be created");
        repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Completed,
                None,
            )
            .await
            .expect("operation should complete");

        let error = repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Running,
                Some(&LifecycleOperationPayload::Cleanup(
                    LifecycleCleanupPayload::Runpod(RunpodLifecycleCleanupPayload { step: None }),
                )),
            )
            .await
            .expect_err("terminal operation should not be reopened");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[tokio::test]
    async fn list_running_orders_by_created_at_then_id() {
        let repository = repository("list-running-order").await;
        let first = repository
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("first operation should be created");
        let second = repository
            .create_operation(&"workspace-2".to_string())
            .await
            .expect("second operation should be created");

        sqlx::query("UPDATE lifecycle_operations SET created_at = ?1 WHERE id = ?2")
            .bind("1970-01-01T00:00:02Z")
            .bind(&first.operation_id)
            .execute(&repository.pool)
            .await
            .expect("first timestamp update should succeed");
        sqlx::query("UPDATE lifecycle_operations SET created_at = ?1 WHERE id = ?2")
            .bind("1970-01-01T00:00:01Z")
            .bind(&second.operation_id)
            .execute(&repository.pool)
            .await
            .expect("second timestamp update should succeed");

        let running = repository
            .list_running()
            .await
            .expect("running list should succeed");

        assert_eq!(running[0].operation_id, second.operation_id);
        assert_eq!(running[1].operation_id, first.operation_id);
    }

    #[tokio::test]
    async fn decoded_persisted_row_with_blank_operation_id_is_corrupt() {
        let repository = repository("blank-operation-id").await;
        let payload = provision_payload(None);
        let payload_json = encode_payload(Some(&payload)).expect("payload should encode");
        let now = timestamp().expect("timestamp should format");

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
                 VALUES ('', 'workspace-1', 'running', ?1, ?2, ?3, NULL)",
        )
        .bind(payload_json)
        .bind(&now)
        .bind(&now)
        .execute(&repository.pool)
        .await
        .expect("corrupt row insert should succeed");

        let error = repository
            .list_running()
            .await
            .expect_err("blank persisted operation id should be corrupt");

        assert!(matches!(error, LifecycleJournalError::DataInvalid { .. }));
    }
}
