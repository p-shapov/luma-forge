use sqlx::{Row, SqlitePool};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    domain::lifecycle_operation::{
        LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
        LifecycleOperationState, WorkspaceId,
    },
    shared::AppFuture,
};

use super::{
    payload::{decode_payload, encode_payload},
    repository::LifecycleJournalRepository,
    LifecycleJournalError,
};

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
                    LifecycleJournalError::QueryFailed
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
            .map_err(|_| LifecycleJournalError::QueryFailed)?;

            row.as_ref().map(row_to_operation).transpose()
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
            .map_err(|_| LifecycleJournalError::QueryFailed)?;

            rows.iter().map(row_to_operation).collect()
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
            .map_err(|_| LifecycleJournalError::QueryFailed)?;

            row.as_ref().map(row_to_operation).transpose()
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
                .map_err(|_| LifecycleJournalError::QueryFailed)?;

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
                    LifecycleJournalError::QueryFailed
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
                    LifecycleJournalError::QueryFailed
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
        .map_err(|_| LifecycleJournalError::QueryFailed)?;

        row.as_ref()
            .map(row_to_operation)
            .transpose()?
            .ok_or(LifecycleJournalError::OperationNotFound)
    }
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), LifecycleJournalError> {
    if workspace_id.trim().is_empty() {
        Err(LifecycleJournalError::Corrupt)
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

fn validate_persisted_operation_id(operation_id: &str) -> Result<(), LifecycleJournalError> {
    if operation_id.trim().is_empty() {
        Err(LifecycleJournalError::Corrupt)
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
        Err(LifecycleJournalError::Corrupt)
    } else {
        Ok(())
    }
}

fn state_to_storage(state: LifecycleOperationState) -> &'static str {
    match state {
        LifecycleOperationState::Running => "running",
        LifecycleOperationState::Completed => "completed",
        LifecycleOperationState::Failed => "failed",
        LifecycleOperationState::Stale => "stale",
    }
}

fn state_from_storage(state: &str) -> Result<LifecycleOperationState, LifecycleJournalError> {
    match state {
        "running" => Ok(LifecycleOperationState::Running),
        "completed" => Ok(LifecycleOperationState::Completed),
        "failed" => Ok(LifecycleOperationState::Failed),
        "stale" => Ok(LifecycleOperationState::Stale),
        _ => Err(LifecycleJournalError::Corrupt),
    }
}

fn timestamp() -> Result<String, LifecycleJournalError> {
    format_timestamp(OffsetDateTime::now_utc())
}

fn format_timestamp(timestamp: OffsetDateTime) -> Result<String, LifecycleJournalError> {
    timestamp
        .format(&Rfc3339)
        .map_err(|_| LifecycleJournalError::QueryFailed)
}

fn parse_timestamp(timestamp: &str) -> Result<OffsetDateTime, LifecycleJournalError> {
    OffsetDateTime::parse(timestamp, &Rfc3339).map_err(|_| LifecycleJournalError::Corrupt)
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
            Err(LifecycleJournalError::Corrupt)
        } else {
            Ok(Some(finished_at))
        }
    } else {
        Ok(Some(updated_at))
    }
}

fn row_to_operation(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<LifecycleOperation, LifecycleJournalError> {
    let operation_id = row
        .try_get::<String, _>("id")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let workspace_id = row
        .try_get::<String, _>("workspace_id")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let state = row
        .try_get::<String, _>("state")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let payload_json = row
        .try_get::<String, _>("payload_json")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let created_at = row
        .try_get::<String, _>("created_at")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let updated_at = row
        .try_get::<String, _>("updated_at")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;
    let finished_at = row
        .try_get::<Option<String>, _>("finished_at")
        .map_err(|_| LifecycleJournalError::SchemaMismatch)?;

    validate_persisted_operation_id(&operation_id)?;
    validate_workspace_id(&workspace_id)?;

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

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error
            .code()
            .is_some_and(|code| code.as_ref() == "1555" || code.as_ref() == "2067"),
        _ => false,
    }
}

#[cfg(test)]
pub mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::{sqlite::SqliteConnectOptions, Row};

    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleOperationPayload, LifecycleOperationState,
                ProvisionedRemoteLifecycleOperationPayload,
            },
            provisioned_remote::ProviderApiError,
            provisioned_remote::{ProvisionedRemoteLifecycleError, ProvisionedRemoteProvisionStep},
        },
        lifecycle_journal::schema,
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
        schema::bootstrap(&pool)
            .await
            .expect("journal schema should bootstrap");

        SqliteLifecycleJournalRepository::new(pool)
    }

    fn provision_payload(
        step: Option<ProvisionedRemoteProvisionStep>,
        error: Option<ProvisionedRemoteLifecycleError>,
    ) -> LifecycleOperationPayload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision { step, error },
        )
    }

    #[tokio::test]
    async fn schema_creates_lifecycle_table_indexes_and_unique_running_constraint() {
        let repository = repository("schema").await;

        let table = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'lifecycle_operations'",
        )
        .fetch_optional(&repository.pool)
        .await
        .expect("table query should succeed");
        assert!(table.is_some());

        let indexes = sqlx::query("PRAGMA index_list(lifecycle_operations)")
            .fetch_all(&repository.pool)
            .await
            .expect("index list should succeed");
        assert!(indexes
            .iter()
            .any(|row| row.get::<String, _>("name") == "idx_lifecycle_operations_workspace_id"));
        assert!(indexes
            .iter()
            .any(|row| row.get::<String, _>("name") == "idx_lifecycle_operations_state"));

        let running_index = indexes
            .iter()
            .find(|row| {
                row.get::<String, _>("name") == "idx_lifecycle_operations_running_workspace_unique"
            })
            .expect("running unique index should exist");
        assert_eq!(running_index.get::<i64, _>("unique"), 1);
        assert_eq!(running_index.get::<i64, _>("partial"), 1);

        let predicate: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master
             WHERE type = 'index' AND name = 'idx_lifecycle_operations_running_workspace_unique'",
        )
        .fetch_one(&repository.pool)
        .await
        .expect("index sql should exist");
        assert!(predicate.ends_with("WHERE state = 'running'"));
    }

    #[tokio::test]
    async fn creating_second_running_operation_for_workspace_returns_running_operation_exists() {
        let repository = repository("duplicate-running").await;
        let payload = provision_payload(None, None);

        repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("first operation should be created");
        let error = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect_err("second running operation should fail");

        assert_eq!(error, LifecycleJournalError::RunningOperationExists);
    }

    #[tokio::test]
    async fn completed_operation_allows_new_running_operation_for_same_workspace() {
        let repository = repository("completed-allows-new").await;
        let payload = provision_payload(None, None);
        let operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("operation should be created");

        repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Completed,
                &payload,
            )
            .await
            .expect("operation should complete");
        let next_operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("new running operation should be created");

        assert_ne!(operation.operation_id, next_operation.operation_id);
        assert_eq!(next_operation.state, LifecycleOperationState::Running);
    }

    #[tokio::test]
    async fn find_list_and_latest_return_expected_running_rows() {
        let repository = repository("queries").await;
        let payload = provision_payload(None, None);
        let workspace_1 = "workspace-1".to_string();
        let workspace_2 = "workspace-2".to_string();
        let first = repository
            .create_operation(&workspace_1, &payload)
            .await
            .expect("first operation should be created");
        let second = repository
            .create_operation(&workspace_2, &payload)
            .await
            .expect("second operation should be created");

        let found = repository
            .find_running_by_workspace(&workspace_1)
            .await
            .expect("running lookup should succeed")
            .expect("running operation should exist");
        let running = repository
            .list_running()
            .await
            .expect("running list should succeed");
        let latest = repository
            .latest_for_workspace(&workspace_1)
            .await
            .expect("latest lookup should succeed")
            .expect("latest operation should exist");

        assert_eq!(found.operation_id, first.operation_id);
        assert_eq!(latest.operation_id, first.operation_id);
        assert_eq!(running.len(), 2);
        assert!(running
            .iter()
            .any(|operation| operation.operation_id == first.operation_id));
        assert!(running
            .iter()
            .any(|operation| operation.operation_id == second.operation_id));
    }

    #[tokio::test]
    async fn delete_for_workspace_removes_only_matching_rows() {
        let repository = repository("delete-for-workspace").await;
        let payload = provision_payload(None, None);
        let workspace_1 = "workspace-1".to_string();
        let workspace_2 = "workspace-2".to_string();
        repository
            .create_operation(&workspace_1, &payload)
            .await
            .expect("first operation should be created");
        let remaining = repository
            .create_operation(&workspace_2, &payload)
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
        let initial_payload = provision_payload(None, None);
        let finished_payload = provision_payload(
            Some(ProvisionedRemoteProvisionStep::CreateVolume),
            Some(ProvisionedRemoteLifecycleError::ProviderApiFailed {
                reason: ProviderApiError::RateLimited,
            }),
        );
        let operation = repository
            .create_operation(&"workspace-1".to_string(), &initial_payload)
            .await
            .expect("operation should be created");

        let marked = repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Failed,
                &finished_payload,
            )
            .await
            .expect("operation should be marked failed");

        assert_eq!(marked.state, LifecycleOperationState::Failed);
        assert_eq!(marked.payload, finished_payload);
        assert!(marked.finished_at.is_some());
    }

    #[tokio::test]
    async fn update_operation_persists_operation_updated_at_timestamp() {
        let repository = repository("update-operation-updated-at").await;
        let payload = provision_payload(None, None);
        let mut operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("operation should be created");
        let updated_at = OffsetDateTime::from_unix_timestamp(42).expect("valid timestamp");
        operation.updated_at = updated_at;

        let updated = repository
            .update_operation(&operation)
            .await
            .expect("operation should be updated");

        assert_eq!(updated.updated_at, updated_at);
    }

    #[tokio::test]
    async fn update_operation_cannot_change_workspace_id_or_created_at() {
        let repository = repository("update-operation-immutable-fields").await;
        let payload = provision_payload(None, None);
        let operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
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

        assert_eq!(workspace_error, LifecycleJournalError::Corrupt);
        assert_eq!(created_at_error, LifecycleJournalError::Corrupt);
    }

    #[tokio::test]
    async fn terminal_update_with_no_finished_at_uses_operation_updated_at() {
        let repository = repository("terminal-update-finished-at").await;
        let payload = provision_payload(None, None);
        let mut operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
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
        let payload = provision_payload(None, None);
        let mut operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
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

        assert_eq!(error, LifecycleJournalError::Corrupt);
    }

    #[tokio::test]
    async fn mark_state_cannot_reopen_terminal_operation() {
        let repository = repository("mark-state-terminal").await;
        let payload = provision_payload(None, None);
        let operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("operation should be created");
        repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Completed,
                &payload,
            )
            .await
            .expect("operation should complete");

        let error = repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Running,
                &payload,
            )
            .await
            .expect_err("terminal operation should not be reopened");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[tokio::test]
    async fn list_running_orders_by_created_at_then_id() {
        let repository = repository("list-running-order").await;
        let payload = provision_payload(None, None);
        let first = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("first operation should be created");
        let second = repository
            .create_operation(&"workspace-2".to_string(), &payload)
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
        let payload = provision_payload(None, None);
        let payload_json = encode_payload(&payload).expect("payload should encode");
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

        assert_eq!(error, LifecycleJournalError::Corrupt);
    }

    #[tokio::test]
    async fn payload_round_trip_preserves_provisioned_remote_kind_step_and_error_detail() {
        let repository = repository("payload-round-trip").await;
        let payload = provision_payload(
            Some(ProvisionedRemoteProvisionStep::PollProvisioner),
            Some(ProvisionedRemoteLifecycleError::ProviderApiFailed {
                reason: ProviderApiError::Timeout,
            }),
        );

        let created = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("operation should be created");
        let found = repository
            .find_running_by_workspace(&"workspace-1".to_string())
            .await
            .expect("running lookup should succeed")
            .expect("running operation should exist");

        assert_eq!(found.operation_id, created.operation_id);
        assert_eq!(found.payload, payload);
    }
}
