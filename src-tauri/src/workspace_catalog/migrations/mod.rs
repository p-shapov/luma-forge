use sqlx::{Row, SqliteTransaction};

use crate::workspace_setup::error::WorkspaceSetupError;

pub(crate) const CURRENT_PERSISTENCE_VERSION: i64 = 2;
pub(crate) const PERSISTENCE_VERSION_KEY: &str = "persistence_version";

pub(super) async fn run(
    transaction: &mut SqliteTransaction<'_>,
) -> Result<(), WorkspaceSetupError> {
    create_schema(transaction).await?;
    let version = persistence_version(transaction).await?;
    if version > CURRENT_PERSISTENCE_VERSION {
        return Err(WorkspaceSetupError::WorkspaceCatalogMigrationFailed);
    }
    set_persistence_version(transaction, CURRENT_PERSISTENCE_VERSION).await?;

    Ok(())
}

async fn create_schema(transaction: &mut SqliteTransaction<'_>) -> Result<(), WorkspaceSetupError> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            gpu_cloud_provider_id TEXT NOT NULL,
            lifecycle_state TEXT NOT NULL,
            workflow_preset_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            workspace_json TEXT NOT NULL
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_workspaces_lifecycle_state ON workspaces(lifecycle_state)",
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_workspaces_workflow_preset_id ON workspaces(workflow_preset_id)",
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_catalog_metadata (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    Ok(())
}

async fn persistence_version(
    transaction: &mut SqliteTransaction<'_>,
) -> Result<i64, WorkspaceSetupError> {
    let value: Option<String> = sqlx::query(
        r#"
        SELECT value
        FROM workspace_catalog_metadata
        WHERE key = ?
        "#,
    )
    .bind(PERSISTENCE_VERSION_KEY)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?
    .map(|row| row.try_get("value"))
    .transpose()
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    value
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)
}

async fn set_persistence_version(
    transaction: &mut SqliteTransaction<'_>,
    version: i64,
) -> Result<(), WorkspaceSetupError> {
    sqlx::query(
        r#"
        INSERT INTO workspace_catalog_metadata (key, value)
        VALUES (?, ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        "#,
    )
    .bind(PERSISTENCE_VERSION_KEY)
    .bind(version.to_string())
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    Ok(())
}
