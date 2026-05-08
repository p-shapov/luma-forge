use crate::workspace::workspace_setup::workspace_setup_tests::sample_workspace;

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
