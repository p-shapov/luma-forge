use crate::workspace_setup::tests::sample_workspace;

use serde_json::json;
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use super::*;

#[tokio::test]
async fn lists_empty_catalog() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");

    assert_eq!(
        catalog.persistence_version().await,
        CURRENT_PERSISTENCE_VERSION
    );
    assert!(catalog
        .list_workspaces()
        .await
        .expect("list")
        .workspaces
        .is_empty());
}

#[tokio::test]
async fn inserts_and_rereads_workspace() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");

    let created = catalog.insert_workspace(&workspace).await.expect("insert");

    assert_eq!(created, workspace);
    assert_eq!(
        catalog.list_workspaces().await.expect("list").workspaces,
        vec![workspace]
    );
}

#[tokio::test]
async fn already_current_catalog_is_reused_without_rewriting_workspace_json() {
    let path = temp_catalog_path("already-current");
    let catalog = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let original_json = workspace_json(&catalog.pool, &workspace.id).await;
    catalog.pool.close().await;

    let reopened = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect("reopened catalog");

    assert_eq!(
        reopened.persistence_version().await,
        CURRENT_PERSISTENCE_VERSION
    );
    assert_eq!(
        workspace_json(&reopened.pool, &workspace.id).await,
        original_json
    );

    reopened.pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn rejects_future_persistence_version() {
    let path = temp_catalog_path("future-version");
    let catalog = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect("catalog");
    set_persistence_version(&catalog.pool, CURRENT_PERSISTENCE_VERSION + 1).await;
    catalog.pool.close().await;

    let error = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect_err("future version should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn rejects_provider_id_column_mismatch() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    update_workspace_column(&catalog, "gpu_cloud_provider_id", "other-provider").await;

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("provider id mismatch should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn rejects_id_column_mismatch() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    update_workspace_column(&catalog, "id", "018f6a40-0000-7000-8000-000000000002").await;

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("id mismatch should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn rejects_name_column_mismatch() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    update_workspace_column(&catalog, "name", "Other workspace").await;

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("name mismatch should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn rejects_lifecycle_state_column_mismatch() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    update_workspace_column(&catalog, "lifecycle_state", "ready").await;

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("lifecycle state mismatch should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn rejects_workflow_preset_id_column_mismatch() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    update_workspace_column(&catalog, "workflow_preset_id", "other-preset").await;

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("workflow preset id mismatch should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn stores_provider_id_column_from_workspace() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");

    catalog.insert_workspace(&workspace).await.expect("insert");

    let row =
        sqlx::query("SELECT gpu_cloud_provider_id, workspace_json FROM workspaces WHERE id = ?")
            .bind(&workspace.id)
            .fetch_one(&catalog.pool)
            .await
            .expect("provider id row");
    let provider_id: String = row.try_get("gpu_cloud_provider_id").expect("provider id");
    let workspace_json: String = row.try_get("workspace_json").expect("workspace json");
    let stored_workspace: Workspace =
        serde_json::from_str(&workspace_json).expect("workspace json should decode");

    assert_eq!(provider_id, "runpod");
    assert_eq!(
        stored_workspace.gpu_cloud_provider_id,
        workspace.gpu_cloud_provider_id
    );
}

#[tokio::test]
async fn rejects_duplicate_workspace_id() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog
        .insert_workspace(&workspace)
        .await
        .expect("first insert");

    let error = catalog
        .insert_workspace(&workspace)
        .await
        .expect_err("duplicate should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceAlreadyExists);
}

#[tokio::test]
async fn maps_decode_failure() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    sqlx::query(
        r#"
            INSERT INTO workspaces (
                id,
                name,
                gpu_cloud_provider_id,
                lifecycle_state,
                workflow_preset_id,
                created_at,
                updated_at,
                workspace_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
    )
    .bind("bad")
    .bind("Bad")
    .bind("runpod")
    .bind("draft")
    .bind("preset")
    .bind("2026-05-08T00:00:00Z")
    .bind("2026-05-08T00:00:00Z")
    .bind("{bad json")
    .execute(&catalog.pool)
    .await
    .expect("insert bad payload");

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("bad json should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn rejects_invalid_workspace_payload() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    let mut payload = serde_json::to_value(&workspace).expect("workspace json value");
    payload["persistent_storage_volume_snapshot"] = json!({
        "gpu_cloud_provider_id": "runpod",
        "provider_resource_id": "volume-1",
        "datacenter_id": "EU-RO-1",
        "provider_resource_status": "ready",
        "provisioned_size_bytes": 1,
        "mount_path": "/workspace"
    });
    sqlx::query("UPDATE workspaces SET workspace_json = ? WHERE id = ?")
        .bind(payload.to_string())
        .bind(&workspace.id)
        .execute(&catalog.pool)
        .await
        .expect("update workspace payload");

    let error = catalog
        .list_workspaces()
        .await
        .expect_err("invalid workspace payload should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
}

#[tokio::test]
async fn migrates_legacy_workspace_json() {
    let path = temp_catalog_path("legacy-workspace");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    let legacy_json = legacy_workspace_json(&workspace);
    create_unversioned_catalog(&path, &workspace, legacy_json).await;

    let catalog = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect("catalog should migrate");

    assert_eq!(
        catalog.persistence_version().await,
        CURRENT_PERSISTENCE_VERSION
    );
    assert_eq!(
        catalog.list_workspaces().await.expect("list").workspaces,
        vec![workspace.clone()]
    );
    assert_eq!(
        serde_json::from_str::<Workspace>(&workspace_json(&catalog.pool, &workspace.id).await)
            .expect("migrated workspace json"),
        workspace
    );

    catalog.pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn failed_legacy_migration_does_not_record_version() {
    let path = temp_catalog_path("failed-legacy-workspace");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    let mut legacy = legacy_workspace_value(&workspace);
    legacy["placement_plan"]["selected_workflow_preset"]["id"] = json!("missing-preset");
    create_unversioned_catalog(&path, &workspace, legacy.to_string()).await;

    let error = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect_err("catalog migration should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogUnavailable);
    assert!(!metadata_table_exists(&path).await);

    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn duplicate_after_legacy_migration_returns_duplicate_error() {
    let path = temp_catalog_path("legacy-duplicate");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    create_unversioned_catalog(&path, &workspace, legacy_workspace_json(&workspace)).await;
    let catalog = SqliteWorkspaceCatalog::connect(&path, test_migration_source())
        .await
        .expect("catalog should migrate");

    let error = catalog
        .insert_workspace(&workspace)
        .await
        .expect_err("duplicate should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceAlreadyExists);

    catalog.pool.close().await;
    std::fs::remove_file(path).ok();
}

async fn update_workspace_column(catalog: &SqliteWorkspaceCatalog, column: &str, value: &str) {
    let query = match column {
        "id" => "UPDATE workspaces SET id = ?",
        "name" => "UPDATE workspaces SET name = ?",
        "gpu_cloud_provider_id" => "UPDATE workspaces SET gpu_cloud_provider_id = ?",
        "lifecycle_state" => "UPDATE workspaces SET lifecycle_state = ?",
        "workflow_preset_id" => "UPDATE workspaces SET workflow_preset_id = ?",
        _ => panic!("unsupported workspace column"),
    };

    sqlx::query(query)
        .bind(value)
        .execute(&catalog.pool)
        .await
        .expect("update workspace column");
}

impl SqliteWorkspaceCatalog {
    async fn persistence_version(&self) -> i64 {
        sqlx::query(
            r#"
            SELECT value
            FROM workspace_catalog_metadata
            WHERE key = ?
            "#,
        )
        .bind(PERSISTENCE_VERSION_KEY)
        .fetch_one(&self.pool)
        .await
        .expect("version row")
        .try_get::<String, _>("value")
        .expect("version value")
        .parse()
        .expect("version integer")
    }
}

fn temp_catalog_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("luma-forge-{name}-{}.sqlite", uuid::Uuid::new_v4()))
}

async fn create_unversioned_catalog(
    path: &std::path::Path,
    workspace: &Workspace,
    workspace_json: String,
) {
    let pool = connect_test_pool(path).await;
    sqlx::query(
        r#"
        CREATE TABLE workspaces (
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
    .execute(&pool)
    .await
    .expect("create old workspaces table");

    sqlx::query(
        r#"
        INSERT INTO workspaces (
            id,
            name,
            gpu_cloud_provider_id,
            lifecycle_state,
            workflow_preset_id,
            created_at,
            updated_at,
            workspace_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&workspace.id)
    .bind(&workspace.name)
    .bind(gpu_cloud_provider_id_value(
        &workspace.gpu_cloud_provider_id,
    ))
    .bind(lifecycle_state_value(&workspace.lifecycle_state))
    .bind(&workspace.placement_plan.selected_workflow_preset().id)
    .bind("2026-05-08T00:00:00Z")
    .bind("2026-05-08T00:00:00Z")
    .bind(workspace_json)
    .execute(&pool)
    .await
    .expect("insert legacy workspace");
    pool.close().await;
}

async fn metadata_table_exists(path: &std::path::Path) -> bool {
    let pool = connect_test_pool(path).await;
    let exists = sqlx::query(
        r#"
        SELECT name
        FROM sqlite_master
        WHERE type = 'table' AND name = 'workspace_catalog_metadata'
        "#,
    )
    .fetch_optional(&pool)
    .await
    .expect("metadata table query")
    .is_some();
    pool.close().await;
    exists
}

async fn workspace_json(pool: &SqlitePool, id: &str) -> String {
    sqlx::query("SELECT workspace_json FROM workspaces WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("workspace json row")
        .try_get("workspace_json")
        .expect("workspace json")
}

async fn set_persistence_version(pool: &SqlitePool, version: i64) {
    sqlx::query(
        r#"
        UPDATE workspace_catalog_metadata
        SET value = ?
        WHERE key = ?
        "#,
    )
    .bind(version.to_string())
    .bind(PERSISTENCE_VERSION_KEY)
    .execute(pool)
    .await
    .expect("set persistence version");
}

async fn connect_test_pool(path: &std::path::Path) -> SqlitePool {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("test catalog parent");
    }
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await
        .expect("test sqlite pool")
}

fn legacy_workspace_json(workspace: &Workspace) -> String {
    legacy_workspace_value(workspace).to_string()
}

fn legacy_workspace_value(workspace: &Workspace) -> serde_json::Value {
    let mut value = serde_json::to_value(workspace).expect("workspace value");
    value["placement_plan"]
        .as_object_mut()
        .expect("placement plan object")
        .remove("gpu_cloud_provider_id");
    let model_asset = value
        .pointer_mut("/placement_plan/selected_workflow_preset/required_model_assets/0")
        .expect("model asset")
        .as_object_mut()
        .expect("model asset object");
    model_asset.remove("install");
    model_asset.insert("file_size_bytes".to_string(), json!(6_938_040_256_u64));
    replace_docker_image_ref_with_legacy_object(
        &mut value,
        "/placement_plan/selected_provisioning_profile/provisioner_worker_runtime",
    );
    replace_docker_image_ref_with_legacy_object(
        &mut value,
        "/placement_plan/selected_endpoint_profile/endpoint_worker_runtime",
    );
    value
}

fn replace_docker_image_ref_with_legacy_object(value: &mut serde_json::Value, pointer: &str) {
    let runtime = value
        .pointer_mut(pointer)
        .expect("runtime")
        .as_object_mut()
        .expect("runtime object");
    let docker_image_ref = runtime
        .remove("docker_image_ref")
        .expect("docker image ref");
    runtime.insert(
        "docker_image".to_string(),
        json!({
            "docker_image_ref": docker_image_ref,
            "docker_image_digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        }),
    );
}
