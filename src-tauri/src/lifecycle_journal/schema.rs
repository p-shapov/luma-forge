use sqlx::{Executor, SqlitePool};

use crate::lifecycle_journal::{errors::storage_unavailable_error, LifecycleJournalError};

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), LifecycleJournalError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS lifecycle_operations (
            id TEXT PRIMARY KEY NOT NULL,
            workspace_id TEXT NOT NULL,
            state TEXT NOT NULL,
            payload_json TEXT NOT NULL,
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
