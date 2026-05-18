use std::{future::Future, path::Path, pin::Pin};

use sqlx::{
    sqlite::{SqlitePoolOptions, SqliteRow},
    Row, SqlitePool,
};

use crate::{
    domain::{
        provider_setup::GpuCloudProviderId,
        workspace::validator as workspace_validator,
        workspace::{Workspace, WorkspaceCatalog, WorkspaceLifecycleState},
    },
    workspace_catalog::{migrations, repository::WorkspaceCatalogRepository},
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalog {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalog {
    pub(crate) async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceSetupError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogStorageUnavailable)?;
        }

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogStorageUnavailable)?;
        let catalog = Self { pool };
        catalog.migrate().await?;
        Ok(catalog)
    }

    async fn migrate(&self) -> Result<(), WorkspaceSetupError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

        migrations::run(&mut transaction).await?;

        transaction
            .commit()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

        Ok(())
    }

    async fn find_workspace(&self, id: &str) -> Result<Option<Workspace>, WorkspaceSetupError> {
        let Some(row) = sqlx::query(
            r#"
            SELECT
                id,
                name,
                gpu_cloud_provider_id,
                lifecycle_state,
                workflow_preset_id,
                workspace_json
            FROM workspaces
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?
        else {
            return Ok(None);
        };

        decode_workspace_row(&row).map(Some)
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            let rows = sqlx::query(
                r#"
                SELECT
                    id,
                    name,
                    gpu_cloud_provider_id,
                    lifecycle_state,
                    workflow_preset_id,
                    workspace_json
                FROM workspaces
                ORDER BY created_at ASC
                "#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            let mut workspaces = Vec::with_capacity(rows.len());
            for row in rows {
                workspaces.push(decode_workspace_row(&row)?);
            }

            let catalog = WorkspaceCatalog { workspaces };
            workspace_validator::validate_workspace_catalog(&catalog)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;

            Ok(catalog)
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move { self.find_workspace(id).await })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            workspace_validator::validate_workspace(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            if self.find_workspace(&workspace.id).await?.is_some() {
                return Err(WorkspaceSetupError::WorkspaceAlreadyExists);
            }

            let now = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            let workspace_json = serde_json::to_string(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            let lifecycle_state = lifecycle_state_value(&workspace.lifecycle_state);

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
            .bind(lifecycle_state)
            .bind(&workspace.placement_plan.selected_workflow_preset().id)
            .bind(&now)
            .bind(&now)
            .bind(workspace_json)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    WorkspaceSetupError::WorkspaceAlreadyExists
                } else {
                    WorkspaceSetupError::WorkspaceCatalogQueryFailed
                }
            })?;

            self.find_workspace(&workspace.id)
                .await?
                .ok_or(WorkspaceSetupError::WorkspaceCatalogQueryFailed)
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            workspace_validator::validate_workspace(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;

            let mut transaction = self
                .pool
                .begin()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            let now = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            let workspace_json = serde_json::to_string(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
            let lifecycle_state = lifecycle_state_value(&workspace.lifecycle_state);

            let result = sqlx::query(
                r#"
                UPDATE workspaces
                SET
                    name = ?,
                    gpu_cloud_provider_id = ?,
                    lifecycle_state = ?,
                    workflow_preset_id = ?,
                    updated_at = ?,
                    workspace_json = ?
                WHERE id = ?
                "#,
            )
            .bind(&workspace.name)
            .bind(gpu_cloud_provider_id_value(
                &workspace.gpu_cloud_provider_id,
            ))
            .bind(lifecycle_state)
            .bind(&workspace.placement_plan.selected_workflow_preset().id)
            .bind(&now)
            .bind(workspace_json)
            .bind(&workspace.id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            if result.rows_affected() != 1 {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
                return Err(WorkspaceSetupError::WorkspaceCatalogQueryFailed);
            }

            let row = sqlx::query(
                r#"
                SELECT
                    id,
                    name,
                    gpu_cloud_provider_id,
                    lifecycle_state,
                    workflow_preset_id,
                    workspace_json
                FROM workspaces
                WHERE id = ?
                "#,
            )
            .bind(&workspace.id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
            let updated = decode_workspace_row(&row)?;

            transaction
                .commit()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            Ok(updated)
        })
    }
}

fn gpu_cloud_provider_id_value(provider_id: &GpuCloudProviderId) -> &'static str {
    match provider_id {
        GpuCloudProviderId::Runpod => "runpod",
    }
}

fn lifecycle_state_value(lifecycle_state: &WorkspaceLifecycleState) -> &'static str {
    match lifecycle_state {
        WorkspaceLifecycleState::Draft => "draft",
        WorkspaceLifecycleState::Provisioning => "provisioning",
        WorkspaceLifecycleState::Ready => "ready",
        WorkspaceLifecycleState::Failed => "failed",
    }
}

pub(super) fn decode_workspace_row(row: &SqliteRow) -> Result<Workspace, WorkspaceSetupError> {
    let workspace_json: String = row
        .try_get("workspace_json")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let workspace: Workspace = serde_json::from_str(&workspace_json)
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;

    workspace_validator::validate_workspace(&workspace)
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;
    validate_workspace_row(row, &workspace)?;

    Ok(workspace)
}

pub(super) fn validate_workspace_row(
    row: &SqliteRow,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    let id: String = row
        .try_get("id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let name: String = row
        .try_get("name")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let gpu_cloud_provider_id: String = row
        .try_get("gpu_cloud_provider_id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let lifecycle_state: String = row
        .try_get("lifecycle_state")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;
    let workflow_preset_id: String = row
        .try_get("workflow_preset_id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)?;

    if id != workspace.id
        || name != workspace.name
        || gpu_cloud_provider_id != gpu_cloud_provider_id_value(&workspace.gpu_cloud_provider_id)
        || lifecycle_state != lifecycle_state_value(&workspace.lifecycle_state)
        || workflow_preset_id != workspace.placement_plan.selected_workflow_preset().id
    {
        return Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch);
    }

    Ok(())
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        placement::PlacementPlan,
        runtime::ResolvedRuntimeImageSnapshot,
        workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderResourceStatus, WorkspaceLifecycleState,
        },
    };

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn catalog_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join("luma-forge-workspace-catalog-tests")
            .join(format!("{name}-{}.sqlite", uuid::Uuid::new_v4()))
    }

    fn workflow_preset(id: &str) -> WorkflowPreset {
        WorkflowPreset {
            id: id.to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI Text to Image".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 80 * 1024 * 1024 * 1024,
            runtime_contract: RuntimeContractReference {
                id: "comfyui-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![],
            required_custom_nodes: vec![],
        }
    }

    fn placement_plan(workflow_preset_id: &str) -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA A40".to_string(),
            persistent_storage_volume_size_bytes: 80 * 1024 * 1024 * 1024,
            endpoint_keep_alive_seconds: 5,
            selected_workflow_preset: workflow_preset(workflow_preset_id),
        }
    }

    fn runtime_snapshot() -> ResolvedRuntimeImageSnapshot {
        ResolvedRuntimeImageSnapshot {
            contract_id: "comfyui-python312-cu121".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_image_ref: format!("ghcr.io/luma-forge/provisioner@sha256:{DIGEST_A}"),
            endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
        }
    }

    fn draft_workspace(id: &str, name: &str, workflow_preset_id: &str) -> Workspace {
        Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            id.to_string(),
            name.to_string(),
            placement_plan(workflow_preset_id),
            runtime_snapshot(),
        )
        .expect("valid draft workspace")
    }

    fn volume(status: ProviderResourceStatus) -> PersistentStorageVolumeSnapshot {
        PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-id".to_string(),
            provider_resource_status: status,
            mount_path: "/workspace".to_string(),
        }
    }

    async fn insert_workspace_row(
        catalog: &SqliteWorkspaceCatalog,
        workspace: &Workspace,
        workspace_json: String,
    ) {
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
        .bind("2026-05-18T00:00:00Z")
        .bind("2026-05-18T00:00:00Z")
        .bind(workspace_json)
        .execute(&catalog.pool)
        .await
        .expect("insert workspace row");
    }

    #[tokio::test]
    async fn connect_migrates_new_database_and_lists_empty_catalog() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("empty"))
            .await
            .expect("connect catalog");

        let version: String = sqlx::query(
            r#"
            SELECT value
            FROM workspace_catalog_metadata
            WHERE key = ?
            "#,
        )
        .bind(migrations::PERSISTENCE_VERSION_KEY)
        .fetch_one(&catalog.pool)
        .await
        .expect("read persistence version")
        .try_get("value")
        .expect("version value");
        assert_eq!(version, migrations::CURRENT_PERSISTENCE_VERSION.to_string());

        let index_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM sqlite_master
            WHERE type = 'index'
                AND name IN (
                    'idx_workspaces_lifecycle_state',
                    'idx_workspaces_workflow_preset_id'
                )
            "#,
        )
        .fetch_one(&catalog.pool)
        .await
        .expect("read indexes")
        .try_get("count")
        .expect("index count");
        assert_eq!(index_count, 2);

        let listed = catalog.list_workspaces().await.expect("list workspaces");
        assert!(listed.workspaces.is_empty());
        assert_eq!(
            catalog
                .find_workspace_by_id("missing")
                .await
                .expect("find missing workspace"),
            None
        );
    }

    #[tokio::test]
    async fn insert_find_list_and_reconnect_round_trip_workspace() {
        let path = catalog_path("round-trip");
        let catalog = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");

        let inserted = catalog
            .insert_workspace(&workspace)
            .await
            .expect("insert workspace");
        assert_eq!(inserted, workspace);
        assert_eq!(
            catalog
                .find_workspace_by_id("workspace-a")
                .await
                .expect("find workspace"),
            Some(workspace.clone())
        );
        assert_eq!(
            catalog.list_workspaces().await.expect("list workspaces"),
            WorkspaceCatalog {
                workspaces: vec![workspace.clone()]
            }
        );

        let reconnected = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect("reconnect catalog");
        assert_eq!(
            reconnected
                .find_workspace_by_id("workspace-a")
                .await
                .expect("find persisted workspace"),
            Some(workspace)
        );
    }

    #[tokio::test]
    async fn insert_rejects_duplicate_workspace_id_without_replacing_existing_row() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("duplicate"))
            .await
            .expect("connect catalog");
        let original = draft_workspace("workspace-a", "Original", "preset-a");
        let duplicate = draft_workspace("workspace-a", "Duplicate", "preset-b");

        catalog
            .insert_workspace(&original)
            .await
            .expect("insert original workspace");
        assert_eq!(
            catalog.insert_workspace(&duplicate).await,
            Err(WorkspaceSetupError::WorkspaceAlreadyExists)
        );
        assert_eq!(
            catalog
                .find_workspace_by_id("workspace-a")
                .await
                .expect("find original workspace"),
            Some(original)
        );
    }

    #[tokio::test]
    async fn insert_rejects_invalid_workspace_without_persisting_row() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("invalid-insert"))
            .await
            .expect("connect catalog");
        let invalid = Workspace {
            name: " ".to_string(),
            ..draft_workspace("workspace-a", "Workspace A", "preset-a")
        };

        assert_eq!(
            catalog.insert_workspace(&invalid).await,
            Err(WorkspaceSetupError::WorkspaceCatalogCorrupt)
        );
        assert!(catalog
            .list_workspaces()
            .await
            .expect("list workspaces")
            .workspaces
            .is_empty());
    }

    #[tokio::test]
    async fn update_persists_existing_workspace_and_derived_columns() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("update"))
            .await
            .expect("connect catalog");
        let mut workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");
        catalog
            .insert_workspace(&workspace)
            .await
            .expect("insert workspace");

        workspace.name = "Updated Workspace".to_string();
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        let updated = catalog
            .update_workspace(&workspace)
            .await
            .expect("update workspace");
        assert_eq!(updated, workspace);

        let row = sqlx::query(
            r#"
            SELECT name, lifecycle_state, workflow_preset_id
            FROM workspaces
            WHERE id = ?
            "#,
        )
        .bind("workspace-a")
        .fetch_one(&catalog.pool)
        .await
        .expect("read workspace row");
        let name: String = row.try_get("name").expect("name");
        let lifecycle_state: String = row.try_get("lifecycle_state").expect("lifecycle_state");
        let workflow_preset_id: String = row
            .try_get("workflow_preset_id")
            .expect("workflow_preset_id");
        assert_eq!(name, "Updated Workspace");
        assert_eq!(lifecycle_state, "provisioning");
        assert_eq!(workflow_preset_id, "preset-a");
        assert_eq!(
            catalog
                .find_workspace_by_id("workspace-a")
                .await
                .expect("find updated workspace"),
            Some(workspace)
        );
    }

    #[tokio::test]
    async fn update_missing_workspace_fails_without_inserting() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("missing-update"))
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");

        assert_eq!(
            catalog.update_workspace(&workspace).await,
            Err(WorkspaceSetupError::WorkspaceCatalogQueryFailed)
        );
        assert!(catalog
            .list_workspaces()
            .await
            .expect("list workspaces")
            .workspaces
            .is_empty());
    }

    #[tokio::test]
    async fn find_and_list_report_corrupt_workspace_json() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("corrupt-json"))
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");
        insert_workspace_row(&catalog, &workspace, "{not valid json".to_string()).await;

        assert_eq!(
            catalog.find_workspace_by_id("workspace-a").await,
            Err(WorkspaceSetupError::WorkspaceCatalogCorrupt)
        );
        assert_eq!(
            catalog.list_workspaces().await,
            Err(WorkspaceSetupError::WorkspaceCatalogCorrupt)
        );
    }

    #[tokio::test]
    async fn find_and_list_report_schema_mismatch_between_columns_and_json() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("schema-mismatch"))
            .await
            .expect("connect catalog");
        let workspace = draft_workspace("workspace-a", "Workspace A", "preset-a");
        let workspace_json = serde_json::to_string(&workspace).expect("serialize workspace");
        insert_workspace_row(&catalog, &workspace, workspace_json).await;
        sqlx::query("UPDATE workspaces SET lifecycle_state = ? WHERE id = ?")
            .bind("ready")
            .bind("workspace-a")
            .execute(&catalog.pool)
            .await
            .expect("corrupt lifecycle column");

        assert_eq!(
            catalog.find_workspace_by_id("workspace-a").await,
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
        assert_eq!(
            catalog.list_workspaces().await,
            Err(WorkspaceSetupError::WorkspaceCatalogSchemaMismatch)
        );
    }

    #[tokio::test]
    async fn list_orders_workspaces_by_created_at() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("ordering"))
            .await
            .expect("connect catalog");
        let first_inserted = draft_workspace("workspace-a", "Workspace A", "preset-a");
        let second_inserted = draft_workspace("workspace-b", "Workspace B", "preset-b");
        catalog
            .insert_workspace(&first_inserted)
            .await
            .expect("insert first workspace");
        catalog
            .insert_workspace(&second_inserted)
            .await
            .expect("insert second workspace");
        sqlx::query("UPDATE workspaces SET created_at = ? WHERE id = ?")
            .bind("2026-05-18T00:00:02Z")
            .bind("workspace-a")
            .execute(&catalog.pool)
            .await
            .expect("update first created_at");
        sqlx::query("UPDATE workspaces SET created_at = ? WHERE id = ?")
            .bind("2026-05-18T00:00:01Z")
            .bind("workspace-b")
            .execute(&catalog.pool)
            .await
            .expect("update second created_at");

        let listed = catalog.list_workspaces().await.expect("list workspaces");
        assert_eq!(listed.workspaces, vec![second_inserted, first_inserted]);
    }

    #[tokio::test]
    async fn migration_rejects_database_newer_than_current_version() {
        let path = catalog_path("future-version");
        let catalog = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect("connect catalog");
        sqlx::query(
            r#"
            UPDATE workspace_catalog_metadata
            SET value = ?
            WHERE key = ?
            "#,
        )
        .bind((migrations::CURRENT_PERSISTENCE_VERSION + 1).to_string())
        .bind(migrations::PERSISTENCE_VERSION_KEY)
        .execute(&catalog.pool)
        .await
        .expect("set future persistence version");
        catalog.pool.close().await;

        let error = SqliteWorkspaceCatalog::connect(&path)
            .await
            .expect_err("future persistence version should fail migration");
        assert_eq!(error, WorkspaceSetupError::WorkspaceCatalogMigrationFailed);
    }
}
