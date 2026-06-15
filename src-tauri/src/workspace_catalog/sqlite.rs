use sqlx::{Row, SqlitePool};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::{
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceCatalog, WorkspaceRuntime, WorkspaceState},
    },
    shared::AppFuture,
};

use super::{
    errors::{
        data_invalid_error, data_invalid_message, schema_invalid_error, storage_unavailable_error,
        WorkspaceCatalogError,
    },
    repository::WorkspaceCatalogRepository,
};

const LIST_WORKSPACES_SQL: &str =
    "SELECT id, state, workflow_id, workflow_version, runtime_json FROM workspaces ORDER BY created_at ASC";
const FIND_WORKSPACE_SQL: &str =
    "SELECT id, state, workflow_id, workflow_version, runtime_json FROM workspaces WHERE id = ?1";

struct PersistedWorkspace<'a> {
    workspace: &'a Workspace,
    runtime_json: String,
    state: &'static str,
}

impl<'a> PersistedWorkspace<'a> {
    fn encode(workspace: &'a Workspace) -> Result<Self, WorkspaceCatalogError> {
        validate_id(&workspace.id)?;
        validate_workflow_reference(&workspace.workflow)?;

        let runtime_json = serde_json::to_string(&workspace.runtime).map_err(data_invalid_error)?;
        let state = workspace_state_columns(&workspace.state);

        Ok(Self {
            workspace,
            runtime_json,
            state: state.state,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalogRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            let rows = sqlx::query(LIST_WORKSPACES_SQL)
                .fetch_all(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;
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

            let row = sqlx::query(FIND_WORKSPACE_SQL)
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_unavailable_error)?;

            row.as_ref().map(workspace_from_row).transpose()
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            let persisted = PersistedWorkspace::encode(workspace)?;
            let now = timestamp()?;

            sqlx::query(
                "INSERT INTO workspaces (
                    id,
                    state,
                    workflow_id,
                    workflow_version,
                    runtime_json,
                    created_at,
                    updated_at
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(&persisted.workspace.id)
            .bind(persisted.state)
            .bind(&persisted.workspace.workflow.id)
            .bind(&persisted.workspace.workflow.version)
            .bind(&persisted.runtime_json)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    WorkspaceCatalogError::WorkspaceAlreadyExists
                } else {
                    storage_unavailable_error(error)
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
            let persisted = PersistedWorkspace::encode(workspace)?;
            let now = timestamp()?;

            let result = sqlx::query(
                "UPDATE workspaces
                 SET state = ?1,
                     workflow_id = ?2,
                     workflow_version = ?3,
                     runtime_json = ?4,
                     updated_at = ?5
                 WHERE id = ?6",
            )
            .bind(persisted.state)
            .bind(&persisted.workspace.workflow.id)
            .bind(&persisted.workspace.workflow.version)
            .bind(&persisted.runtime_json)
            .bind(now)
            .bind(&persisted.workspace.id)
            .execute(&self.pool)
            .await
            .map_err(storage_unavailable_error)?;

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
                .map_err(storage_unavailable_error)?;

            if result.rows_affected() == 0 {
                return Err(WorkspaceCatalogError::WorkspaceNotFound);
            }

            Ok(())
        })
    }
}

pub(super) fn validate_id(id: &str) -> Result<(), WorkspaceCatalogError> {
    if id.trim().is_empty() {
        Err(data_invalid_message("ID is empty"))
    } else {
        Ok(())
    }
}

pub(super) fn validate_workflow_reference(
    workflow: &WorkflowReference,
) -> Result<(), WorkspaceCatalogError> {
    validate_required_text(&workflow.id, "workflow ID is missing")?;
    validate_required_text(&workflow.version, "workflow version is missing")
}

fn validate_required_text(value: &str, message: &'static str) -> Result<(), WorkspaceCatalogError> {
    if value.trim().is_empty() {
        Err(data_invalid_message(message))
    } else {
        Ok(())
    }
}

fn workspace_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Workspace, WorkspaceCatalogError> {
    let id = required_text(row, "id")?;
    let state = required_text(row, "state")?;
    let workflow_id = required_text(row, "workflow_id")?;
    let workflow_version = required_text(row, "workflow_version")?;
    let runtime_json = required_text(row, "runtime_json")?;
    validate_id(&id)?;

    let workflow = WorkflowReference {
        id: workflow_id,
        version: workflow_version,
    };
    validate_workflow_reference(&workflow)?;

    Ok(Workspace {
        id,
        workflow,
        state: workspace_state_from_columns(&state)?,
        runtime: serde_json::from_str::<WorkspaceRuntime>(&runtime_json)
            .map_err(data_invalid_error)?,
    })
}

fn required_text(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<String, WorkspaceCatalogError> {
    row.try_get(column).map_err(schema_invalid_error)
}

struct WorkspaceStateColumns {
    state: &'static str,
}

fn workspace_state_columns(state: &WorkspaceState) -> WorkspaceStateColumns {
    match state {
        WorkspaceState::NotProvisioned => WorkspaceStateColumns {
            state: "not_provisioned",
        },
        WorkspaceState::Ready => WorkspaceStateColumns { state: "ready" },
        WorkspaceState::CleanupRequired => WorkspaceStateColumns {
            state: "cleanup_required",
        },
        WorkspaceState::Invalid => WorkspaceStateColumns { state: "invalid" },
    }
}

fn workspace_state_from_columns(state: &str) -> Result<WorkspaceState, WorkspaceCatalogError> {
    match state {
        "not_provisioned" => Ok(WorkspaceState::NotProvisioned),
        "ready" => Ok(WorkspaceState::Ready),
        "cleanup_required" => Ok(WorkspaceState::CleanupRequired),
        "invalid" => Ok(WorkspaceState::Invalid),
        state => Err(data_invalid_message(format!("unknown state: {state}"))),
    }
}

fn timestamp() -> Result<String, WorkspaceCatalogError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(storage_unavailable_error)
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
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::domain::{
        runpod::placement::RunpodPlacementPlan,
        runpod::runtime::{RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    };

    use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};

    use super::*;

    fn catalog_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-{name}-{nonce}.sqlite"))
    }

    async fn open_repository(path: impl AsRef<Path>) -> SqliteWorkspaceCatalogRepository {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .expect("repository connection should succeed");

        crate::workspace_catalog::schema::bootstrap(&pool)
            .await
            .expect("workspace schema bootstrap should succeed");

        SqliteWorkspaceCatalogRepository::new(pool)
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

    async fn index_exists(pool: &SqlitePool, index_name: &str) -> bool {
        sqlx::query("SELECT name FROM sqlite_master WHERE type = 'index' AND name = ?1")
            .bind(index_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master index query should succeed")
            .is_some()
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
        state: &'a str,
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
                    state,
                    workflow_id,
                    workflow_version,
                    runtime_json,
                    created_at,
                    updated_at
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(row.id)
        .bind(row.state)
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
            runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
                placement: RunpodPlacementPlan {
                    data_center_id: "datacenter-1".to_string(),
                    gpu_type_id: "gpu-1".to_string(),
                    volume_size_gb: 19,
                },
                resources: RunpodResources {
                    network_volume_id: None,
                    provisioner_pod_id: None,
                    endpoint_id: None,
                    template_id: None,
                },
            }),
        }
    }

    #[tokio::test]
    async fn bootstrap_creates_normalized_workspace_columns_and_indexes() {
        let path = catalog_path("schema");

        let repository = open_repository(&path).await;
        let pool = repository.pool.clone();

        assert!(table_exists(&pool, "workspaces").await);

        assert_text_not_null_column(&pool, "workspaces", "id").await;
        assert_text_not_null_column(&pool, "workspaces", "state").await;
        assert_text_not_null_column(&pool, "workspaces", "workflow_id").await;
        assert_text_not_null_column(&pool, "workspaces", "workflow_version").await;
        assert_text_not_null_column(&pool, "workspaces", "runtime_json").await;
        assert_text_not_null_column(&pool, "workspaces", "created_at").await;
        assert_text_not_null_column(&pool, "workspaces", "updated_at").await;

        assert!(index_exists(&pool, "idx_workspaces_state").await);

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn insert_workspace_stores_runtime_json_without_workspace_state_duplication() {
        let repository = open_repository(catalog_path("insert")).await;
        let workspace = workspace("workspace-1");

        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");

        let row = sqlx::query(
            "SELECT state, workflow_id, workflow_version, runtime_json
                 FROM workspaces WHERE id = ?1",
        )
        .bind("workspace-1")
        .fetch_one(&repository.pool.clone())
        .await
        .expect("workspace row should exist");

        assert_eq!(row.get::<String, _>("state"), "not_provisioned");
        assert_eq!(row.get::<String, _>("workflow_id"), "preset");
        assert_eq!(row.get::<String, _>("workflow_version"), "1");

        let runtime_json = row.get::<String, _>("runtime_json");
        let runtime_value: serde_json::Value =
            serde_json::from_str(&runtime_json).expect("runtime json should parse");
        assert_eq!(
            runtime_value
                .get("runtime_type")
                .and_then(|value| value.as_str()),
            Some("runpod")
        );
        assert!(runtime_value.get("state").is_none());
        assert!(runtime_value.get("placement").is_some());
        assert!(runtime_value.get("resources").is_some());
    }

    #[tokio::test]
    async fn list_workspaces_returns_empty_catalog() {
        let path = catalog_path("empty-catalog");

        let repository = open_repository(&path).await;

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

        let repository = open_repository(&path).await;

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
    async fn workspace_states_round_trip() {
        let path = catalog_path("state-round-trip");
        let mut ready = workspace("ready");
        ready.state = WorkspaceState::Ready;
        let mut cleanup_required = workspace("cleanup-required");
        cleanup_required.state = WorkspaceState::CleanupRequired;
        let mut invalid = workspace("invalid");
        invalid.state = WorkspaceState::Invalid;

        let repository = open_repository(&path).await;
        repository
            .insert_workspace(&ready)
            .await
            .expect("ready insert should succeed");
        repository
            .insert_workspace(&cleanup_required)
            .await
            .expect("cleanup required insert should succeed");
        repository
            .insert_workspace(&invalid)
            .await
            .expect("invalid insert should succeed");

        let found_ready = repository
            .find_workspace_by_id("ready")
            .await
            .expect("ready find should succeed");
        let found_cleanup_required = repository
            .find_workspace_by_id("cleanup-required")
            .await
            .expect("cleanup required find should succeed");
        let found_invalid = repository
            .find_workspace_by_id("invalid")
            .await
            .expect("invalid find should succeed");

        assert_eq!(found_ready, Some(ready));
        assert_eq!(found_cleanup_required, Some(cleanup_required));
        assert_eq!(found_invalid, Some(invalid));

        let cleanup_required_row = sqlx::query("SELECT state FROM workspaces WHERE id = ?1")
            .bind("cleanup-required")
            .fetch_one(&repository.pool.clone())
            .await
            .expect("cleanup required row should exist");
        let invalid_row = sqlx::query("SELECT state FROM workspaces WHERE id = ?1")
            .bind("invalid")
            .fetch_one(&repository.pool.clone())
            .await
            .expect("invalid row should exist");

        assert_eq!(
            cleanup_required_row.get::<String, _>("state"),
            "cleanup_required"
        );
        assert_eq!(invalid_row.get::<String, _>("state"), "invalid");

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn unknown_state_returns_corrupt() {
        let path = catalog_path("unknown-state");
        let workspace = workspace("workspace-1");
        let runtime_json = serde_json::to_string(&workspace.runtime)
            .expect("runtime serialization should succeed");

        let repository = open_repository(&path).await;

        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "unknown-state",
                state: "unknown",
                workflow_id: "preset",
                workflow_version: "1",
                runtime_json: &runtime_json,
                created_at: "2026-06-06T00:00:01Z",
                updated_at: "2026-06-06T00:00:01Z",
            },
        )
        .await;

        let error = repository
            .find_workspace_by_id("unknown-state")
            .await
            .expect_err("unknown state should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "unknown state: unknown".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn empty_workflow_id_returns_corrupt() {
        let path = catalog_path("empty-workflow-id");
        let workspace = workspace("workspace-1");
        let runtime_json = serde_json::to_string(&workspace.runtime)
            .expect("runtime serialization should succeed");

        let repository = open_repository(&path).await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                state: "not_provisioned",
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

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "workflow ID is missing".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn empty_workflow_version_returns_corrupt() {
        let path = catalog_path("empty-workflow-version");
        let workspace = workspace("workspace-1");
        let runtime_json = serde_json::to_string(&workspace.runtime)
            .expect("runtime serialization should succeed");

        let repository = open_repository(&path).await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                state: "not_provisioned",
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

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "workflow version is missing".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn corrupt_runtime_json_returns_corrupt() {
        let path = catalog_path("corrupt-runtime-json");

        let repository = open_repository(&path).await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-1",
                state: "not_provisioned",
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

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "EOF while parsing an object at line 1 column 1".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn empty_stored_workspace_id_returns_corrupt() {
        let path = catalog_path("empty-workspace-id");
        let workspace = workspace("workspace-1");
        let runtime_json = serde_json::to_string(&workspace.runtime)
            .expect("runtime serialization should succeed");

        let repository = open_repository(&path).await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "",
                state: "not_provisioned",
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
            .expect_err("empty stored workspace id should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "ID is empty".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }
    #[tokio::test]
    async fn list_workspaces_orders_by_created_at() {
        let path = catalog_path("created-at-order");
        let workspace_1 = workspace("workspace-1");
        let workspace_2 = workspace("workspace-2");
        let runtime_1_json = serde_json::to_string(&workspace_1.runtime)
            .expect("runtime serialization should succeed");
        let runtime_2_json = serde_json::to_string(&workspace_2.runtime)
            .expect("runtime serialization should succeed");

        let repository = open_repository(&path).await;
        insert_workspace_row(
            &repository.pool,
            WorkspaceRowInsert {
                id: "workspace-2",
                state: "not_provisioned",
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
                state: "not_provisioned",
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

        let repository = open_repository(&path).await;
        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");
        drop(repository);

        let repository = open_repository(&path).await;
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

        let repository = open_repository(&path).await;

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

        let repository = open_repository(&path).await;

        let error = repository
            .find_workspace_by_id(" \t\n")
            .await
            .expect_err("blank id should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "ID is empty".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn insert_workspace_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-insert");
        let workspace = workspace(" ");

        let repository = open_repository(&path).await;

        let error = repository
            .insert_workspace(&workspace)
            .await
            .expect_err("blank workspace id should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "ID is empty".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn update_replaces_existing_workspace() {
        let path = catalog_path("update");
        let mut workspace = workspace("workspace-1");

        let repository = open_repository(&path).await;
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

        let repository = open_repository(&path).await;
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

        let repository = open_repository(&path).await;

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

        let repository = open_repository(&path).await;

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

        let repository = open_repository(&path).await;

        let error = repository
            .update_workspace(&workspace)
            .await
            .expect_err("blank workspace id should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "ID is empty".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_workspace_rejects_blank_id_as_corrupt() {
        let path = catalog_path("blank-delete");

        let repository = open_repository(&path).await;

        let error = repository
            .delete_workspace(" \t\n")
            .await
            .expect_err("blank id should fail");

        assert_eq!(
            error,
            WorkspaceCatalogError::DataInvalid {
                message: "ID is empty".to_string()
            }
        );

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn update_sql_failure_returns_query_failed() {
        let path = catalog_path("update-sql-failure");
        let workspace = workspace("workspace-1");

        let repository = open_repository(&path).await;
        sqlx::query("DROP TABLE workspaces")
            .execute(&repository.pool)
            .await
            .expect("drop table should succeed");

        let error = repository
            .update_workspace(&workspace)
            .await
            .expect_err("update should fail when workspaces table is missing");

        assert!(matches!(
            error,
            WorkspaceCatalogError::StorageUnavailable { .. }
        ));

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn delete_sql_failure_returns_query_failed() {
        let path = catalog_path("delete-sql-failure");

        let repository = open_repository(&path).await;
        sqlx::query("DROP TABLE workspaces")
            .execute(&repository.pool)
            .await
            .expect("drop table should succeed");

        let error = repository
            .delete_workspace("workspace-1")
            .await
            .expect_err("delete should fail when workspaces table is missing");

        assert!(matches!(
            error,
            WorkspaceCatalogError::StorageUnavailable { .. }
        ));

        drop(repository);
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn duplicate_insert_returns_workspace_already_exists() {
        let path = catalog_path("duplicate");
        let workspace = workspace("workspace-1");

        let repository = open_repository(&path).await;
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
