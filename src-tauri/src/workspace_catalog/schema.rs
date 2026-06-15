use sqlx::{Executor, SqlitePool};

use super::errors::{storage_unavailable_error, WorkspaceCatalogError};

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), WorkspaceCatalogError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT NOT NULL PRIMARY KEY,
            runtime_type TEXT NOT NULL,
            state TEXT NOT NULL,
            workflow_id TEXT NOT NULL,
            workflow_version TEXT NOT NULL,
            runtime_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .map_err(storage_unavailable_error)?;

    pool.execute(
        "CREATE INDEX IF NOT EXISTS idx_workspaces_runtime_type ON workspaces (runtime_type)",
    )
    .await
    .map_err(storage_unavailable_error)?;
    pool.execute("CREATE INDEX IF NOT EXISTS idx_workspaces_state ON workspaces (state)")
        .await
        .map_err(storage_unavailable_error)?;

    Ok(())
}
