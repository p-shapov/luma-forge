use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::sqlite::SqliteConnectOptions;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperationPayload, LifecycleOperationState},
        runpod::{RunpodLifecycleOperationPayload, RunpodProvisionStep},
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

fn provision_payload(step: Option<RunpodProvisionStep>) -> LifecycleOperationPayload {
    LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision { step })
}

#[tokio::test]
async fn creating_second_running_operation_for_workspace_returns_running_operation_exists() {
    let repository = repository("duplicate-running").await;
    let payload = provision_payload(None);

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
    let payload = provision_payload(None);
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
async fn delete_for_workspace_removes_only_matching_rows() {
    let repository = repository("delete-for-workspace").await;
    let payload = provision_payload(None);
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
    let initial_payload = provision_payload(None);
    let finished_payload = provision_payload(Some(RunpodProvisionStep::CreateNetworkVolume));
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
async fn update_operation_cannot_change_workspace_id_or_created_at() {
    let repository = repository("update-operation-immutable-fields").await;
    let payload = provision_payload(None);
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
    let payload = provision_payload(None);
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
    let payload = provision_payload(None);
    let mut operation = repository
        .create_operation(&"workspace-1".to_string(), &payload)
        .await
        .expect("operation should be created");
    operation.state = LifecycleOperationState::Failed;
    operation.updated_at = OffsetDateTime::from_unix_timestamp(42).expect("valid timestamp");
    operation.finished_at = Some(OffsetDateTime::from_unix_timestamp(41).expect("valid timestamp"));

    let error = repository
        .update_operation(&operation)
        .await
        .expect_err("finished_at before updated_at should be corrupt");

    assert!(matches!(error, LifecycleJournalError::DataInvalid { .. }));
}

#[tokio::test]
async fn mark_state_cannot_reopen_terminal_operation() {
    let repository = repository("mark-state-terminal").await;
    let payload = provision_payload(None);
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
    let payload = provision_payload(None);
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
    let payload = provision_payload(None);
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

    assert!(matches!(error, LifecycleJournalError::DataInvalid { .. }));
}
