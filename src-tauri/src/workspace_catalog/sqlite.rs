use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::workspace::{Workspace, WorkspaceCatalog},
    shared::{is_blank, AppFuture},
};

use super::{errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository, schema};

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
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalogRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            let rows =
                sqlx::query("SELECT id, workspace_json FROM workspaces ORDER BY created_at ASC")
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

            let row = sqlx::query("SELECT id, workspace_json FROM workspaces WHERE id = ?1")
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

            let workspace_json =
                serde_json::to_string(workspace).map_err(|_| WorkspaceCatalogError::Corrupt)?;
            let now = timestamp()?;

            sqlx::query(
                "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&workspace.id)
            .bind(workspace_json)
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

            let workspace_json =
                serde_json::to_string(workspace).map_err(|_| WorkspaceCatalogError::Corrupt)?;
            let now = timestamp()?;

            let result = sqlx::query(
                "UPDATE workspaces
                 SET workspace_json = ?1, updated_at = ?2
                 WHERE id = ?3",
            )
            .bind(workspace_json)
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
    if is_blank(id) {
        Err(WorkspaceCatalogError::Corrupt)
    } else {
        Ok(())
    }
}

fn workspace_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Workspace, WorkspaceCatalogError> {
    let id = row
        .try_get::<String, _>("id")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workspace_json = row
        .try_get::<String, _>("workspace_json")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workspace: Workspace =
        serde_json::from_str(&workspace_json).map_err(|_| WorkspaceCatalogError::Corrupt)?;

    if workspace.id != id {
        return Err(WorkspaceCatalogError::Corrupt);
    }

    Ok(workspace)
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
        placement::{
            Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
            RemotePlacementPlan,
        },
        provider::GpuCloudProviderId,
        runtime_contract::RuntimeContractReference,
        workflow_preset::{
            ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
            RemoteRuntimeRequirements, WorkflowExecutionType, WorkflowPreset,
        },
        workspace::{
            RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
            RemoteWorkspaceResources, Workspace, WorkspaceRuntime,
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

    fn workspace(id: &str) -> Workspace {
        Workspace {
            id: id.to_string(),
            workflow_preset: WorkflowPreset {
                id: "workflow-1".to_string(),
                version: "1".to_string(),
                name: "Workflow 1".to_string(),
                execution_type: WorkflowExecutionType::T2i,
                requires_hugging_face_api_key: false,
                remote_runtime_requirements: RemoteRuntimeRequirements {
                    required_base_volume_size_bytes: 1,
                    provider_requirements: vec![RemoteProviderRuntimeRequirements {
                        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                        endpoint_contract: RuntimeContractReference {
                            id: "endpoint-contract".to_string(),
                            version: "1".to_string(),
                        },
                        provisioner_contract: RuntimeContractReference {
                            id: "provisioner-contract".to_string(),
                            version: "1".to_string(),
                        },
                    }],
                },
                required_model_assets: vec![ModelAsset {
                    id: "asset-1".to_string(),
                    name: "Asset 1".to_string(),
                    download_source: ModelAssetSource::Huggingface {
                        repository_id: "owner/repository".to_string(),
                        file_path: "model.safetensors".to_string(),
                        revision: "main".to_string(),
                    },
                    install_comfyui_relative_path: "models/model.safetensors".to_string(),
                }],
            },
            runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
                remote_placement: RemotePlacementPlan {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    datacenter_id: "datacenter-1".to_string(),
                    gpu_id: "gpu-1".to_string(),
                    remote_volume_size_bytes: 1,
                    remote_capabilities: RemotePlacementCapabilities {
                        remote_endpoint_keep_alive: Capability::Supported(
                            RemoteEndpointKeepAliveLimits {
                                default_seconds: 60,
                                min_seconds: 0,
                                max_seconds: 3600,
                            },
                        ),
                    },
                },
                remote_provisioning: RemoteProvisioningState {
                    status: RemoteProvisioningStatus::NotStarted,
                    percent: None,
                },
                remote_resources: RemoteWorkspaceResources {
                    remote_volume: None,
                    remote_provisioner: None,
                    remote_endpoint: None,
                },
            }),
        }
    }

    #[tokio::test]
    async fn connect_creates_schema() {
        let path = catalog_path("schema");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should create schema");

        assert!(table_exists(&repository.pool, "metadata").await);
        assert!(table_exists(&repository.pool, "workspaces").await);

        let (id_type, _) = column_info(&repository.pool, "workspaces", "id").await;
        assert_eq!(id_type, "TEXT");
        assert_text_not_null_column(&repository.pool, "workspaces", "workspace_json").await;
        assert_text_not_null_column(&repository.pool, "workspaces", "created_at").await;
        assert_text_not_null_column(&repository.pool, "workspaces", "updated_at").await;

        let version = sqlx::query("SELECT value FROM metadata WHERE key = ?1")
            .bind("workspace_catalog_schema_version")
            .fetch_one(&repository.pool)
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
                workspace_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (id, workspace_json)
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

        workspace.workflow_preset.name = "Updated Workflow".to_string();
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
