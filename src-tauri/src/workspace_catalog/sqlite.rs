use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::{
        workflow_preset::WorkflowReference,
        workspace::{
            Workspace, WorkspaceCatalog, WorkspaceCleanupRequiredReason,
            WorkspaceRuntimeInvalidReason, WorkspaceState,
        },
    },
    shared::AppFuture,
};

use super::{
    errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository, runtime, schema,
};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceCatalogError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|_| WorkspaceCatalogError::StorageUnavailable)?;

        schema::bootstrap(&pool).await?;

        Ok(Self { pool })
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> SqlitePool {
        self.pool.clone()
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalogRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            let rows = sqlx::query(
                "SELECT id, runtime_type, provider_id, state, state_reason, workflow_id, workflow_version, runtime_json
                 FROM workspaces ORDER BY created_at ASC",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| WorkspaceCatalogError::QueryFailed)?;
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

            let row = sqlx::query(
                "SELECT id, runtime_type, provider_id, state, state_reason, workflow_id, workflow_version, runtime_json
                 FROM workspaces WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

            row.as_ref().map(workspace_from_row).transpose()
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            validate_id(&workspace.id)?;
            validate_workflow_reference(&workspace.workflow)?;

            let encoded = runtime::encode_runtime(&workspace.runtime)?;
            let provider_id = json_string(&encoded.provider_id)?;
            let state = workspace_state_columns(&workspace.state);
            let now = timestamp()?;

            sqlx::query(
                "INSERT INTO workspaces (
                    id,
                    runtime_type,
                    provider_id,
                    state,
                    state_reason,
                    workflow_id,
                    workflow_version,
                    runtime_json,
                    created_at,
                    updated_at
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .bind(&workspace.id)
            .bind(&encoded.runtime_type)
            .bind(provider_id)
            .bind(state.state)
            .bind(state.reason)
            .bind(&workspace.workflow.id)
            .bind(&workspace.workflow.version)
            .bind(&encoded.runtime_json)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    WorkspaceCatalogError::WorkspaceAlreadyExists
                } else {
                    WorkspaceCatalogError::QueryFailed
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
            validate_id(&workspace.id)?;
            validate_workflow_reference(&workspace.workflow)?;

            let encoded = runtime::encode_runtime(&workspace.runtime)?;
            let provider_id = json_string(&encoded.provider_id)?;
            let state = workspace_state_columns(&workspace.state);
            let now = timestamp()?;

            let result = sqlx::query(
                "UPDATE workspaces
                 SET runtime_type = ?1,
                     provider_id = ?2,
                     state = ?3,
                     state_reason = ?4,
                     workflow_id = ?5,
                     workflow_version = ?6,
                     runtime_json = ?7,
                     updated_at = ?8
                 WHERE id = ?9",
            )
            .bind(&encoded.runtime_type)
            .bind(provider_id)
            .bind(state.state)
            .bind(state.reason)
            .bind(&workspace.workflow.id)
            .bind(&workspace.workflow.version)
            .bind(&encoded.runtime_json)
            .bind(now)
            .bind(&workspace.id)
            .execute(&self.pool)
            .await
            .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

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
                .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

            if result.rows_affected() == 0 {
                return Err(WorkspaceCatalogError::WorkspaceNotFound);
            }

            Ok(())
        })
    }
}

fn validate_id(id: &str) -> Result<(), WorkspaceCatalogError> {
    if id.trim().is_empty() {
        Err(WorkspaceCatalogError::Corrupt)
    } else {
        Ok(())
    }
}

fn validate_workflow_reference(workflow: &WorkflowReference) -> Result<(), WorkspaceCatalogError> {
    validate_id(&workflow.id)?;
    validate_id(&workflow.version)
}

fn workspace_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Workspace, WorkspaceCatalogError> {
    let id = row
        .try_get::<String, _>("id")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let runtime_type = row
        .try_get::<String, _>("runtime_type")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let provider_id = row
        .try_get::<String, _>("provider_id")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let state = row
        .try_get::<String, _>("state")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let state_reason = row
        .try_get::<Option<String>, _>("state_reason")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workflow_id = row
        .try_get::<String, _>("workflow_id")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workflow_version = row
        .try_get::<String, _>("workflow_version")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let runtime_json = row
        .try_get::<String, _>("runtime_json")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workflow = WorkflowReference {
        id: workflow_id,
        version: workflow_version,
    };
    validate_workflow_reference(&workflow)?;
    let state = workspace_state_from_columns(&state, state_reason.as_deref())?;
    let runtime = runtime::decode_runtime(&runtime_type, &runtime_json)?;
    let encoded = runtime::encode_runtime(&runtime)?;

    if json_string(&encoded.provider_id)? != provider_id {
        return Err(WorkspaceCatalogError::Corrupt);
    }

    Ok(Workspace {
        id,
        workflow,
        state,
        runtime,
    })
}

fn json_string<T>(value: &T) -> Result<String, WorkspaceCatalogError>
where
    T: serde::Serialize,
{
    serde_json::to_value(value)
        .map_err(|_| WorkspaceCatalogError::Corrupt)?
        .as_str()
        .map(str::to_string)
        .ok_or(WorkspaceCatalogError::Corrupt)
}

struct WorkspaceStateColumns {
    state: &'static str,
    reason: Option<&'static str>,
}

fn workspace_state_columns(state: &WorkspaceState) -> WorkspaceStateColumns {
    match state {
        WorkspaceState::NotProvisioned => WorkspaceStateColumns {
            state: "not_provisioned",
            reason: None,
        },
        WorkspaceState::Ready => WorkspaceStateColumns {
            state: "ready",
            reason: None,
        },
        WorkspaceState::CleanupRequired { reason } => WorkspaceStateColumns {
            state: "cleanup_required",
            reason: Some(cleanup_required_reason_column(reason)),
        },
        WorkspaceState::Invalid { reason } => WorkspaceStateColumns {
            state: "invalid",
            reason: Some(invalid_reason_column(reason)),
        },
    }
}

fn workspace_state_from_columns(
    state: &str,
    reason: Option<&str>,
) -> Result<WorkspaceState, WorkspaceCatalogError> {
    match (state, reason) {
        ("not_provisioned", None) => Ok(WorkspaceState::NotProvisioned),
        ("ready", None) => Ok(WorkspaceState::Ready),
        ("cleanup_required", Some(reason)) => Ok(WorkspaceState::CleanupRequired {
            reason: cleanup_required_reason_from_column(reason)?,
        }),
        ("invalid", Some(reason)) => Ok(WorkspaceState::Invalid {
            reason: invalid_reason_from_column(reason)?,
        }),
        _ => Err(WorkspaceCatalogError::Corrupt),
    }
}

fn cleanup_required_reason_column(reason: &WorkspaceCleanupRequiredReason) -> &'static str {
    match reason {
        WorkspaceCleanupRequiredReason::ProvisionFailed => "provision_failed",
        WorkspaceCleanupRequiredReason::CleanupFailed => "cleanup_failed",
        WorkspaceCleanupRequiredReason::DeleteFailed => "delete_failed",
        WorkspaceCleanupRequiredReason::OperationInterrupted => "operation_interrupted",
    }
}

fn cleanup_required_reason_from_column(
    reason: &str,
) -> Result<WorkspaceCleanupRequiredReason, WorkspaceCatalogError> {
    match reason {
        "provision_failed" => Ok(WorkspaceCleanupRequiredReason::ProvisionFailed),
        "cleanup_failed" => Ok(WorkspaceCleanupRequiredReason::CleanupFailed),
        "delete_failed" => Ok(WorkspaceCleanupRequiredReason::DeleteFailed),
        "operation_interrupted" => Ok(WorkspaceCleanupRequiredReason::OperationInterrupted),
        _ => Err(WorkspaceCatalogError::Corrupt),
    }
}

fn invalid_reason_column(reason: &WorkspaceRuntimeInvalidReason) -> &'static str {
    match reason {
        WorkspaceRuntimeInvalidReason::OperationInterrupted => "operation_interrupted",
        WorkspaceRuntimeInvalidReason::ProvisionFailed => "provision_failed",
        WorkspaceRuntimeInvalidReason::CleanupFailed => "cleanup_failed",
        WorkspaceRuntimeInvalidReason::DeleteFailed => "delete_failed",
        WorkspaceRuntimeInvalidReason::CorruptRuntimeState => "corrupt_runtime_state",
    }
}

fn invalid_reason_from_column(
    reason: &str,
) -> Result<WorkspaceRuntimeInvalidReason, WorkspaceCatalogError> {
    match reason {
        "operation_interrupted" => Ok(WorkspaceRuntimeInvalidReason::OperationInterrupted),
        "provision_failed" => Ok(WorkspaceRuntimeInvalidReason::ProvisionFailed),
        "cleanup_failed" => Ok(WorkspaceRuntimeInvalidReason::CleanupFailed),
        "delete_failed" => Ok(WorkspaceRuntimeInvalidReason::DeleteFailed),
        "corrupt_runtime_state" => Ok(WorkspaceRuntimeInvalidReason::CorruptRuntimeState),
        _ => Err(WorkspaceCatalogError::Corrupt),
    }
}

fn timestamp() -> Result<String, WorkspaceCatalogError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| WorkspaceCatalogError::QueryFailed)
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
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::domain::{
        provisioned_remote::GpuCloudProviderId,
        provisioned_remote::{ProvisionedRemoteResources, ProvisionedRemoteRuntime},
        provisioned_remote::{RemoteEndpointKeepAliveLimits, RemotePlacementPlan},
        workflow_preset::WorkflowReference,
        workspace::{
            Workspace, WorkspaceCleanupRequiredReason, WorkspaceRuntime,
            WorkspaceRuntimeInvalidReason, WorkspaceState,
        },
    };

    use sqlx::Row;

    use super::*;

    fn catalog_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-{name}-{nonce}.sqlite"))
    }

    async fn table_exists(pool: &SqlitePool, table_name: &str) -> bool {
        let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master query should succeed");

        row.is_some()
    }

    async fn column_info(pool: &SqlitePool, table_name: &str, column_name: &str) -> (String, bool) {
        let row = sqlx::query(&format!("PRAGMA table_info({table_name})"))
            .fetch_all(pool)
            .await
            .expect("table info query should succeed")
            .into_iter()
            .find(|row| row.get::<String, _>("name") == column_name)
            .expect("column should exist");

        let column_type = row.get::<String, _>("type");
        let not_null = row.get::<i64, _>("notnull") == 1;

        (column_type, not_null)
    }

    async fn assert_text_not_null_column(pool: &SqlitePool, table_name: &str, column_name: &str) {
        let (column_type, not_null) = column_info(pool, table_name, column_name).await;

        assert_eq!(column_type, "TEXT");
        assert!(not_null, "{table_name}.{column_name} should be NOT NULL");
    }

    async fn assert_text_nullable_column(pool: &SqlitePool, table_name: &str, column_name: &str) {
        let (column_type, not_null) = column_info(pool, table_name, column_name).await;

        assert_eq!(column_type, "TEXT");
        assert!(!not_null, "{table_name}.{column_name} should be nullable");
    }

    async fn index_exists(pool: &SqlitePool, index_name: &str) -> bool {
        sqlx::query("SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?1")
            .bind(index_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master index query should succeed")
            .is_some()
    }

    async fn metadata_version(path: &Path) -> Option<String> {
        let options = SqliteConnectOptions::new().filename(path);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("metadata check connection should succeed");

        sqlx::query_scalar("SELECT value FROM metadata WHERE key = ?1")
            .bind("workspace_catalog_schema_version")
            .fetch_optional(&pool)
            .await
            .expect("metadata version query should succeed")
    }

    async fn workspace_timestamps(pool: &SqlitePool, id: &str) -> (String, String) {
        let row = sqlx::query("SELECT created_at, updated_at FROM workspaces WHERE id = ?1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("workspace timestamp query should succeed");

        (
            row.get::<String, _>("created_at"),
            row.get::<String, _>("updated_at"),
        )
    }

    struct WorkspaceRowInsert<'a> {
        id: &'a str,
        runtime_type: &'a str,
        provider_id: &'a str,
        state: &'a str,
        state_reason: Option<&'a str>,
        workflow_id: &'a str,
        workflow_version: &'a str,
        runtime_json: &'a str,
        created_at: &'a str,
        updated_at: &'a str,
    }

    async fn insert_workspace_row(pool: &SqlitePool, row: WorkspaceRowInsert<'_>) {
        sqlx::query(
            "INSERT INTO workspaces (
                id,
                runtime_type,
                provider_id,
                state,
                state_reason,
                workflow_id,
                workflow_version,
                runtime_json,
                created_at,
                updated_at
            )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(row.id)
        .bind(row.runtime_type)
        .bind(row.provider_id)
        .bind(row.state)
        .bind(row.state_reason)
        .bind(row.workflow_id)
        .bind(row.workflow_version)
        .bind(row.runtime_json)
        .bind(row.created_at)
        .bind(row.updated_at)
        .execute(pool)
        .await
        .expect("workspace row insert should succeed");
    }

    fn workspace(id: &str) -> Workspace {
        Workspace {
            id: id.to_string(),
            workflow: WorkflowReference {
                id: "preset".to_string(),
                version: "1".to_string(),
            },
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
                placement: RemotePlacementPlan {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    datacenter_id: "datacenter-1".to_string(),
                    gpu_id: "gpu-1".to_string(),
                    volume_size_bytes: 1,
                    keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
                        default_seconds: 60,
                        min_seconds: 0,
                        max_seconds: 3600,
                    }),
                },
                resources: ProvisionedRemoteResources {
                    volume_id: None,
                    provisioner_id: None,
                    endpoint_id: None,
                },
            }),
        }
    }

    #[tokio::test]
    async fn bootstrap_creates_normalized_workspace_columns_and_indexes() {
        let path = catalog_path("schema");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should create schema");
        let pool = repository.pool();

        assert!(table_exists(&pool, "metadata").await);
        assert!(table_exists(&pool, "workspaces").await);

        assert_text_not_null_column(&pool, "workspaces", "id").await;
        assert_text_not_null_column(&pool, "workspaces", "runtime_type").await;
        assert_text_not_null_column(&pool, "workspaces", "provider_id").await;
        assert_text_not_null_column(&pool, "workspaces", "state").await;
        assert_text_nullable_column(&pool, "workspaces", "state_reason").await;
        assert_text_not_null_column(&pool, "workspaces", "workflow_id").await;
        assert_text_not_null_column(&pool, "workspaces", "workflow_version").await;
        assert_text_not_null_column(&pool, "workspaces", "runtime_json").await;
        assert_text_not_null_column(&pool, "workspaces", "created_at").await;
        assert_text_not_null_column(&pool, "workspaces", "updated_at").await;

        assert!(index_exists(&pool, "idx_workspaces_runtime_type").await);
        assert!(index_exists(&pool, "idx_workspaces_provider_id").await);
        assert!(index_exists(&pool, "idx_workspaces_state").await);

        let version = sqlx::query("SELECT value FROM metadata WHERE key = ?1")
            .bind("workspace_catalog_schema_version")
            .fetch_one(&pool)
            .await
            .expect("metadata version should exist")
            .get::<String, _>("value");

        assert_eq!(version, "1");

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_existing_wrong_workspaces_table_without_metadata() {
        let path = catalog_path("wrong-workspaces");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query("CREATE TABLE workspaces (id TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("setup table creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject incompatible schema");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_legacy_workspace_json_table() {
        let path = catalog_path("legacy-workspace-json");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT PRIMARY KEY,
                workspace_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject legacy workspace json schema");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_existing_composite_primary_key_without_metadata() {
        let path = catalog_path("composite-primary-key");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT NOT NULL,
                runtime_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                state TEXT NOT NULL,
                state_reason TEXT,
                workflow_id TEXT NOT NULL,
                workflow_version TEXT NOT NULL,
                runtime_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (id, runtime_type)
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject incompatible primary key");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_existing_current_table_missing_indexes() {
        let path = catalog_path("missing-indexes");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT NOT NULL PRIMARY KEY,
                runtime_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                state TEXT NOT NULL,
                state_reason TEXT,
                workflow_id TEXT NOT NULL,
                workflow_version TEXT NOT NULL,
                runtime_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject missing workspace indexes");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_not_null_state_reason_column() {
        let path = catalog_path("not-null-state-reason");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT NOT NULL PRIMARY KEY,
                runtime_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                state TEXT NOT NULL,
                state_reason TEXT NOT NULL,
                workflow_id TEXT NOT NULL,
                workflow_version TEXT NOT NULL,
                runtime_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject non-null state_reason schema");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_extra_workspace_index() {
        let path = catalog_path("extra-index");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT NOT NULL PRIMARY KEY,
                runtime_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                state TEXT NOT NULL,
                state_reason TEXT,
                workflow_id TEXT NOT NULL,
                workflow_version TEXT NOT NULL,
                runtime_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_runtime_type ON workspaces (runtime_type)")
            .execute(&pool)
            .await
            .expect("setup runtime type index creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_provider_id ON workspaces (provider_id)")
            .execute(&pool)
            .await
            .expect("setup provider id index creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_state ON workspaces (state)")
            .execute(&pool)
            .await
            .expect("setup state index creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_created_at ON workspaces (created_at)")
            .execute(&pool)
            .await
            .expect("setup extra index creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject extra workspace index");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn connect_rejects_partial_workspace_index() {
        let path = catalog_path("partial-index");

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("setup connection should succeed");
        sqlx::query(
            "CREATE TABLE workspaces (
                id TEXT NOT NULL PRIMARY KEY,
                runtime_type TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                state TEXT NOT NULL,
                state_reason TEXT,
                workflow_id TEXT NOT NULL,
                workflow_version TEXT NOT NULL,
                runtime_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("setup table creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_runtime_type ON workspaces (runtime_type)")
            .execute(&pool)
            .await
            .expect("setup runtime type index creation should succeed");
        sqlx::query("CREATE INDEX idx_workspaces_provider_id ON workspaces (provider_id)")
            .execute(&pool)
            .await
            .expect("setup provider id index creation should succeed");
        sqlx::query(
            "CREATE INDEX idx_workspaces_state ON workspaces (state) WHERE state = 'ready'",
        )
        .execute(&pool)
        .await
        .expect("setup partial state index creation should succeed");
        drop(pool);

        let error = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect_err("connect should reject partial workspace index");

        assert_eq!(error, WorkspaceCatalogError::SchemaMismatch);
        assert_eq!(metadata_version(&path).await, None);

        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn insert_workspace_stores_runtime_json_without_workspace_state_duplication() {
        let repository = SqliteWorkspaceCatalogRepository::connect(catalog_path("insert"))
            .await
            .expect("repository should connect");
        let workspace = workspace("workspace-1");

        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");

        let row = sqlx::query(
            "SELECT runtime_type, provider_id, state, state_reason, workflow_id, workflow_version, runtime_json
             FROM workspaces WHERE id = ?1",
        )
        .bind("workspace-1")
        .fetch_one(&repository.pool())
        .await
        .expect("workspace row should exist");

        assert_eq!(row.get::<String, _>("runtime_type"), "provisioned_remote");
        assert_eq!(row.get::<String, _>("provider_id"), "runpod");
        assert_eq!(row.get::<String, _>("state"), "not_provisioned");
        assert_eq!(row.get::<Option<String>, _>("state_reason"), None);
        assert_eq!(row.get::<String, _>("workflow_id"), "preset");
        assert_eq!(row.get::<String, _>("workflow_version"), "1");

        let runtime_json = row.get::<String, _>("runtime_json");
        let runtime_value: serde_json::Value =
            serde_json::from_str(&runtime_json).expect("runtime json should parse");
        assert!(runtime_value.get("resources").is_some());
        assert!(runtime_value.get("state").is_none());
        assert_eq!(
            runtime_value
                .as_object()
                .expect("runtime json should be object")
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["placement".to_string(), "resources".to_string()]
        );
    }

    #[tokio::test]
    async fn list_workspaces_returns_empty_catalog() {
        let path = catalog_path("empty-catalog");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let catalog = repository
            .list_workspaces()
            .await
            .expect("list should succeed");

        assert!(catalog.workspaces.is_empty());

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn insert_list_and_find_round_trip_workspace() {
        let path = catalog_path("round-trip");
        let workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let inserted = repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");
        let catalog = repository
            .list_workspaces()
            .await
            .expect("list should succeed");
        let found = repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed");

        assert_eq!(inserted, workspace);
        assert_eq!(catalog.workspaces, vec![workspace.clone()]);
        assert_eq!(found, Some(workspace));

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn reason_bearing_workspace_states_round_trip() {
        let path = catalog_path("reason-state-round-trip");
        let mut cleanup_required = workspace("cleanup-required");
        cleanup_required.state = WorkspaceState::CleanupRequired {
            reason: WorkspaceCleanupRequiredReason::ProvisionFailed,
        };
        let mut invalid = workspace("invalid");
        invalid.state = WorkspaceState::Invalid {
            reason: WorkspaceRuntimeInvalidReason::CorruptRuntimeState,
        };

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&cleanup_required)
            .await
            .expect("cleanup required insert should succeed");
        repository
            .insert_workspace(&invalid)
            .await
            .expect("invalid insert should succeed");

        let found_cleanup_required = repository
            .find_workspace_by_id("cleanup-required")
            .await
            .expect("cleanup required find should succeed");
        let found_invalid = repository
            .find_workspace_by_id("invalid")
            .await
            .expect("invalid find should succeed");

        assert_eq!(found_cleanup_required, Some(cleanup_required));
        assert_eq!(found_invalid, Some(invalid));

        let cleanup_required_row =
            sqlx::query("SELECT state, state_reason FROM workspaces WHERE id = ?1")
                .bind("cleanup-required")
                .fetch_one(&repository.pool())
                .await
                .expect("cleanup required row should exist");
        let invalid_row = sqlx::query("SELECT state, state_reason FROM workspaces WHERE id = ?1")
            .bind("invalid")
            .fetch_one(&repository.pool())
            .await
            .expect("invalid row should exist");

        assert_eq!(
            cleanup_required_row.get::<String, _>("state"),
            "cleanup_required"
        );
        assert_eq!(
            cleanup_required_row.get::<Option<String>, _>("state_reason"),
            Some("provision_failed".to_string())
        );
        assert_eq!(invalid_row.get::<String, _>("state"), "invalid");
        assert_eq!(
            invalid_row.get::<Option<String>, _>("state_reason"),
            Some("corrupt_runtime_state".to_string())
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn invalid_state_reason_combinations_return_corrupt() {
        let path = catalog_path("invalid-state-reason");
        let workspace = workspace("workspace-1");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = workspace.runtime;
        let runtime_json =
            serde_json::to_string(&runtime).expect("runtime serialization should succeed");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        for (id, state, state_reason) in [
            ("ready-with-reason", "ready", Some("provision_failed")),
            ("cleanup-without-reason", "cleanup_required", None),
            (
                "cleanup-with-invalid-reason",
                "cleanup_required",
                Some("corrupt_runtime_state"),
            ),
            ("invalid-without-reason", "invalid", None),
            ("invalid-with-unknown-reason", "invalid", Some("unknown")),
        ] {
            insert_workspace_row(
                &repository.pool,
                WorkspaceRowInsert {
                    id,
                    runtime_type: "provisioned_remote",
                    provider_id: "runpod",
                    state,
                    state_reason,
                    workflow_id: "preset",
                    workflow_version: "1",
                    runtime_json: &runtime_json,
                    created_at: "2026-06-06T00:00:01Z",
                    updated_at: "2026-06-06T00:00:01Z",
                },
            )
            .await;

            let error = repository
                .find_workspace_by_id(id)
                .await
                .expect_err("invalid state reason combination should fail");

            assert_eq!(error, WorkspaceCatalogError::Corrupt);
        }

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn empty_workflow_id_returns_corrupt() {
        let path = catalog_path("empty-workflow-id");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = workspace("workspace-1").runtime;
        let runtime_json =
            serde_json::to_string(&runtime).expect("runtime serialization should succeed");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                runtime_type: "provisioned_remote",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: "",
                workflow_version: "1",
                runtime_json: &runtime_json,
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let error = repository
            .list_workspaces()
            .await
            .expect_err("empty workflow id should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn empty_workflow_version_returns_corrupt() {
        let path = catalog_path("empty-workflow-version");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = workspace("workspace-1").runtime;
        let runtime_json =
            serde_json::to_string(&runtime).expect("runtime serialization should succeed");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                runtime_type: "provisioned_remote",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: "preset",
                workflow_version: "",
                runtime_json: &runtime_json,
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let error = repository
            .list_workspaces()
            .await
            .expect_err("empty workflow version should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn corrupt_runtime_json_returns_corrupt() {
        let path = catalog_path("corrupt-runtime-json");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                runtime_type: "provisioned_remote",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: "preset",
                workflow_version: "1",
                runtime_json: "{",
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let error = repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect_err("corrupt runtime json should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }
    #[tokio::test]
    async fn unknown_runtime_type_returns_corrupt() {
        let path = catalog_path("unknown-runtime-type");
        let workspace = workspace("workspace-1");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = workspace.runtime;
        let runtime_json =
            serde_json::to_string(&runtime).expect("runtime serialization should succeed");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                runtime_type: "unknown",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: "preset",
                workflow_version: "1",
                runtime_json: &runtime_json,
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let error = repository
            .list_workspaces()
            .await
            .expect_err("unknown runtime type should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn list_workspaces_orders_by_created_at() {
        let path = catalog_path("created-at-order");
        let workspace_1 = workspace("workspace-1");
        let workspace_2 = workspace("workspace-2");
        let WorkspaceRuntime::ProvisionedRemote(runtime_1) = &workspace_1.runtime;
        let runtime_1_json =
            serde_json::to_string(runtime_1).expect("runtime serialization should succeed");
        let WorkspaceRuntime::ProvisionedRemote(runtime_2) = &workspace_2.runtime;
        let runtime_2_json =
            serde_json::to_string(runtime_2).expect("runtime serialization should succeed");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-2",
                runtime_type: "provisioned_remote",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: &workspace_2.workflow.id,
                workflow_version: &workspace_2.workflow.version,
                runtime_json: &runtime_2_json,
                created_at: "2026-06-06T00:00:02Z",
                updated_at: "2026-06-06T00:00:02Z",
            },
        )
        .await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                runtime_type: "provisioned_remote",
                provider_id: "runpod",
                state: "not_provisioned",
                state_reason: None,
                workflow_id: &workspace_1.workflow.id,
                workflow_version: &workspace_1.workflow.version,
                runtime_json: &runtime_1_json,
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let catalog = repository
            .list_workspaces()
            .await
            .expect("list should succeed");

        assert_eq!(catalog.workspaces, vec![workspace_1, workspace_2]);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn persisted_workspace_survives_reconnect() {
        let path = catalog_path("reconnect");
        let workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");
        drop(repository);

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("reconnect should succeed");
        let found = repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed");

        assert_eq!(found, Some(workspace));

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn find_workspace_by_id_returns_none_when_absent() {
        let path = catalog_path("absent-find");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let found = repository
            .find_workspace_by_id("missing-workspace")
            .await
            .expect("find should succeed");

        assert_eq!(found, None);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn find_workspace_by_id_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-find");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .find_workspace_by_id(" \t\n")
            .await
            .expect_err("blank id should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn insert_workspace_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-insert");
        let workspace = workspace(" ");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .insert_workspace(&workspace)
            .await
            .expect_err("blank workspace id should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn update_replaces_existing_workspace() {
        let path = catalog_path("update");
        let mut workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");
        let (created_at_before, _) = workspace_timestamps(&repository.pool, "workspace-1").await;

        workspace.workflow.version = "1.0.1".to_string();
        let updated = repository
            .update_workspace(&workspace)
            .await
            .expect("update should succeed");
        let found = repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed");
        let (created_at_after, _) = workspace_timestamps(&repository.pool, "workspace-1").await;

        assert_eq!(updated, workspace);
        assert_eq!(found, Some(workspace));
        assert_eq!(created_at_after, created_at_before);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_removes_existing_workspace() {
        let path = catalog_path("delete");
        let workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");

        repository
            .delete_workspace("workspace-1")
            .await
            .expect("delete should succeed");
        let found = repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed");

        assert_eq!(found, None);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn missing_update_returns_workspace_not_found() {
        let path = catalog_path("missing-update");
        let workspace = workspace("missing-workspace");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .update_workspace(&workspace)
            .await
            .expect_err("missing update should fail");

        assert_eq!(error, WorkspaceCatalogError::WorkspaceNotFound);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn missing_delete_returns_workspace_not_found() {
        let path = catalog_path("missing-delete");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .delete_workspace("missing-workspace")
            .await
            .expect_err("missing delete should fail");

        assert_eq!(error, WorkspaceCatalogError::WorkspaceNotFound);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn update_workspace_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-update");
        let workspace = workspace(" ");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .update_workspace(&workspace)
            .await
            .expect_err("blank workspace id should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_workspace_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-delete");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");

        let error = repository
            .delete_workspace(" \t\n")
            .await
            .expect_err("blank id should fail");

        assert_eq!(error, WorkspaceCatalogError::Corrupt);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn update_sql_failure_returns_query_failed() {
        let path = catalog_path("update-sql-failure");
        let workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        sqlx::query("DROP TABLE workspaces")
            .execute(&repository.pool)
            .await
            .expect("drop table should succeed");

        let error = repository
            .update_workspace(&workspace)
            .await
            .expect_err("update should fail when workspaces table is missing");

        assert_eq!(error, WorkspaceCatalogError::QueryFailed);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_sql_failure_returns_query_failed() {
        let path = catalog_path("delete-sql-failure");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        sqlx::query("DROP TABLE workspaces")
            .execute(&repository.pool)
            .await
            .expect("drop table should succeed");

        let error = repository
            .delete_workspace("workspace-1")
            .await
            .expect_err("delete should fail when workspaces table is missing");

        assert_eq!(error, WorkspaceCatalogError::QueryFailed);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn duplicate_insert_returns_workspace_already_exists() {
        let path = catalog_path("duplicate");
        let workspace = workspace("workspace-1");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&workspace)
            .await
            .expect("first insert should succeed");

        let error = repository
            .insert_workspace(&workspace)
            .await
            .expect_err("duplicate insert should fail");

        assert_eq!(error, WorkspaceCatalogError::WorkspaceAlreadyExists);

        drop(repository);
        let _ = fs::remove_file(path);
    }
}
