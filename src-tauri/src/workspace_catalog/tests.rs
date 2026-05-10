use crate::workspace_setup::tests::sample_workspace;

use serde_json::json;
use sqlx::Row;

use super::*;

#[tokio::test]
async fn lists_empty_catalog() {
    let catalog = SqliteWorkspaceCatalog::in_memory().await.expect("catalog");

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
