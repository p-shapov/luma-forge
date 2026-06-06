use sqlx::{Executor, SqlitePool};

use super::errors::WorkspaceCatalogError;

const SCHEMA_VERSION_KEY: &str = "workspace_catalog_schema_version";
const SCHEMA_VERSION: &str = "1";

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), WorkspaceCatalogError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    pool.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            workspace_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    sqlx::query("INSERT OR IGNORE INTO metadata (key, value) VALUES (?1, ?2)")
        .bind(SCHEMA_VERSION_KEY)
        .bind(SCHEMA_VERSION)
        .execute(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    let version: Option<String> = sqlx::query_scalar("SELECT value FROM metadata WHERE key = ?1")
        .bind(SCHEMA_VERSION_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    match version.as_deref() {
        Some(SCHEMA_VERSION) => Ok(()),
        _ => Err(WorkspaceCatalogError::SchemaMismatch),
    }
}
