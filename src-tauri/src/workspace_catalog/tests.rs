use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
            RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot,
        },
    },
    workspace_setup::tests::sample_workspace,
};

use serde_json::json;
use sqlx::{Row, SqlitePool};

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
async fn finds_workspace_by_id() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    assert_eq!(
        catalog
            .find_workspace_by_id(&workspace.id)
            .await
            .expect("find"),
        Some(workspace)
    );
    assert_eq!(
        catalog
            .find_workspace_by_id("018f6a40-0000-7000-8000-000000000002")
            .await
            .expect("find missing"),
        None
    );
}

#[tokio::test]
async fn updates_workspace_lifecycle_and_indexed_columns_transactionally() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    workspace.name = "Provisioning workspace".to_string();
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    let updated = catalog
        .update_workspace(&workspace)
        .await
        .expect("update workspace");

    assert_eq!(updated, workspace);
    let row =
        sqlx::query("SELECT name, lifecycle_state, workspace_json FROM workspaces WHERE id = ?")
            .bind(&workspace.id)
            .fetch_one(&catalog.pool)
            .await
            .expect("updated row");
    assert_eq!(
        row.try_get::<String, _>("name").expect("name"),
        "Provisioning workspace"
    );
    assert_eq!(
        row.try_get::<String, _>("lifecycle_state")
            .expect("lifecycle"),
        "provisioning"
    );
    let stored_workspace: Workspace =
        serde_json::from_str(&row.try_get::<String, _>("workspace_json").expect("json"))
            .expect("workspace json");
    assert_eq!(
        stored_workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
}

#[tokio::test]
async fn persists_runpod_snapshots_and_keep_alive_values() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
    workspace.persistent_storage_volume_snapshot = Some(volume_snapshot());
    workspace.provider_provisioning_snapshot = Some(runpod_template_snapshot());
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());

    let updated = catalog
        .update_workspace(&workspace)
        .await
        .expect("persist ready workspace");

    assert_eq!(updated, workspace);
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    let crate::domain::placement::PlacementPlan::Runpod {
        endpoint_keep_alive_seconds,
        ..
    } = stored.placement_plan;
    assert_eq!(endpoint_keep_alive_seconds, 5);
    assert!(stored.provider_provisioning_snapshot.is_some());
}

#[tokio::test]
async fn rejects_update_for_invalid_lifecycle_transition_payload() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");

    workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
    let error = catalog
        .update_workspace(&workspace)
        .await
        .expect_err("ready without snapshots should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogCorrupt);
}

#[tokio::test]
async fn already_current_catalog_is_reused_without_rewriting_workspace_json() {
    let path = temp_catalog_path("already-current");
    let catalog = SqliteWorkspaceCatalog::connect(&path)
        .await
        .expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let original_json = workspace_json(&catalog.pool, &workspace.id).await;
    catalog.pool.close().await;

    let reopened = SqliteWorkspaceCatalog::connect(&path)
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
async fn migrates_v1_workspace_json_to_current_optional_shape() {
    let path = temp_catalog_path("v1-shape");
    let catalog = SqliteWorkspaceCatalog::connect(&path)
        .await
        .expect("catalog");
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let mut payload = serde_json::to_value(&workspace).expect("workspace json value");
    payload
        .as_object_mut()
        .expect("workspace object")
        .remove("provider_provisioning_snapshot");
    sqlx::query("UPDATE workspaces SET workspace_json = ? WHERE id = ?")
        .bind(payload.to_string())
        .bind(&workspace.id)
        .execute(&catalog.pool)
        .await
        .expect("write v1 payload");
    set_persistence_version(&catalog.pool, 1).await;
    catalog.pool.close().await;

    let reopened = SqliteWorkspaceCatalog::connect(&path)
        .await
        .expect("reopened catalog");

    assert_eq!(
        reopened.persistence_version().await,
        CURRENT_PERSISTENCE_VERSION
    );
    assert_eq!(
        reopened
            .find_workspace_by_id(&workspace.id)
            .await
            .expect("find")
            .expect("workspace")
            .provider_provisioning_snapshot,
        None
    );

    reopened.pool.close().await;
    std::fs::remove_file(path).ok();
}

#[tokio::test]
async fn rejects_future_persistence_version() {
    let path = temp_catalog_path("future-version");
    let catalog = SqliteWorkspaceCatalog::connect(&path)
        .await
        .expect("catalog");
    set_persistence_version(&catalog.pool, CURRENT_PERSISTENCE_VERSION + 1).await;
    catalog.pool.close().await;

    let error = SqliteWorkspaceCatalog::connect(&path)
        .await
        .expect_err("future version should fail");

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogMigrationFailed);

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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogCorrupt);
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

    assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogCorrupt);
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

fn volume_snapshot() -> PersistentStorageVolumeSnapshot {
    PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        provisioned_size_bytes: 85899345920,
        mount_path: "/workspace".to_string(),
    }
}

fn endpoint_snapshot() -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        datacenter_id: "EU-RO-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        endpoint_invoke_url: "https://example.invalid/run".to_string(),
    }
}

fn runpod_template_snapshot() -> ProviderProvisioningSnapshot {
    ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: "template-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
            endpoint_worker_image_ref: "ghcr.io/luma-forge/endpoint-worker:dev".to_string(),
            mount_path: "/workspace".to_string(),
        }),
    }
}
