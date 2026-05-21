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
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            environment_prepared_at TEXT
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
        r#"
        CREATE TABLE IF NOT EXISTS workspace_runpod_placements (
            workspace_id TEXT PRIMARY KEY NOT NULL,
            selected_datacenter_id TEXT NOT NULL,
            selected_gpu_id TEXT NOT NULL,
            persistent_storage_volume_size_bytes INTEGER NOT NULL,
            endpoint_keep_alive_seconds INTEGER NOT NULL,
            selected_workflow_preset_id TEXT NOT NULL,
            selected_workflow_preset_json TEXT NOT NULL,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_workspace_runpod_placements_workflow_preset_id ON workspace_runpod_placements(selected_workflow_preset_id)",
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_runtime_images (
            workspace_id TEXT PRIMARY KEY NOT NULL,
            contract_id TEXT NOT NULL,
            contract_version TEXT NOT NULL,
            provisioner_image_ref TEXT NOT NULL,
            endpoint_image_ref TEXT NOT NULL,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_resource_snapshots (
            workspace_id TEXT NOT NULL,
            snapshot_role TEXT NOT NULL,
            gpu_cloud_provider_id TEXT NOT NULL,
            provider_resource_id TEXT NOT NULL,
            provider_resource_status TEXT NOT NULL,
            mount_path TEXT,
            provisioner_status_url TEXT,
            endpoint_invoke_url TEXT,
            PRIMARY KEY(workspace_id, snapshot_role),
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_workspace_resource_snapshots_role ON workspace_resource_snapshots(snapshot_role)",
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_runpod_endpoint_templates (
            workspace_id TEXT PRIMARY KEY NOT NULL,
            template_id TEXT NOT NULL,
            provider_resource_status TEXT NOT NULL,
            endpoint_worker_image_ref TEXT NOT NULL,
            mount_path TEXT NOT NULL,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(&mut **transaction)
    .await
    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workspace_provisioning_failures (
            workspace_id TEXT PRIMARY KEY NOT NULL,
            code TEXT NOT NULL,
            phase TEXT NOT NULL,
            source TEXT NOT NULL,
            retryable INTEGER NOT NULL,
            recovery_action TEXT NOT NULL,
            diagnostic TEXT,
            FOREIGN KEY(workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
        )
        "#,
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, Row};

    async fn migrated_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        let mut transaction = pool.begin().await.expect("begin migration transaction");
        run(&mut transaction).await.expect("run migrations");
        transaction
            .commit()
            .await
            .expect("commit migration transaction");
        pool
    }

    #[tokio::test]
    async fn run_creates_workspace_catalog_schema_and_metadata() {
        let pool = migrated_pool().await;

        let version: String = sqlx::query(
            r#"
            SELECT value
            FROM workspace_catalog_metadata
            WHERE key = ?
            "#,
        )
        .bind(PERSISTENCE_VERSION_KEY)
        .fetch_one(&pool)
        .await
        .expect("read persistence version")
        .try_get("value")
        .expect("version value");
        assert_eq!(version, CURRENT_PERSISTENCE_VERSION.to_string());

        let index_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM sqlite_master
            WHERE type = 'index'
                AND name IN (
                    'idx_workspaces_lifecycle_state',
                    'idx_workspace_runpod_placements_workflow_preset_id',
                    'idx_workspace_resource_snapshots_role'
                )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read indexes")
        .try_get("count")
        .expect("index count");
        assert_eq!(index_count, 3);

        let workspace_json_columns: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM pragma_table_info('workspaces')
            WHERE name = 'workspace_json'
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read workspace columns")
        .try_get("count")
        .expect("workspace_json column count");
        assert_eq!(workspace_json_columns, 0);

        let normalized_table_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM sqlite_master
            WHERE type = 'table'
                AND name IN (
                    'workspace_runpod_placements',
                    'workspace_runtime_images',
                    'workspace_resource_snapshots',
                    'workspace_runpod_endpoint_templates',
                    'workspace_provisioning_failures'
                )
            "#,
        )
        .fetch_one(&pool)
        .await
        .expect("read normalized tables")
        .try_get("count")
        .expect("normalized table count");
        assert_eq!(normalized_table_count, 5);
    }

    #[tokio::test]
    async fn run_rejects_database_newer_than_current_version() {
        let pool = migrated_pool().await;
        sqlx::query(
            r#"
            UPDATE workspace_catalog_metadata
            SET value = ?
            WHERE key = ?
            "#,
        )
        .bind((CURRENT_PERSISTENCE_VERSION + 1).to_string())
        .bind(PERSISTENCE_VERSION_KEY)
        .execute(&pool)
        .await
        .expect("set future persistence version");

        let mut transaction = pool.begin().await.expect("begin migration transaction");
        assert_eq!(
            run(&mut transaction).await,
            Err(WorkspaceSetupError::WorkspaceCatalogMigrationFailed)
        );
    }
}
