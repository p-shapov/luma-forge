use std::{future::Future, path::Path, pin::Pin};

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, SqliteTransaction};

use crate::{
    domain::{
        workspace::validator as workspace_validator,
        workspace::{Workspace, WorkspaceCatalog},
    },
    workspace_catalog::{repository::WorkspaceCatalogRepository, schema_bootstrap},
    workspace_setup::error::WorkspaceSetupError,
};

mod read;
mod values;
mod write;

use read::decode_workspace;
use values::{gpu_cloud_provider_id_value, lifecycle_state_value};
use write::{delete_workspace_details, persist_workspace_details};

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
        catalog.bootstrap_schema().await?;
        Ok(catalog)
    }

    async fn bootstrap_schema(&self) -> Result<(), WorkspaceSetupError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

        schema_bootstrap::bootstrap_and_check(&mut transaction).await?;

        transaction
            .commit()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogMigrationFailed)?;

        Ok(())
    }

    async fn find_workspace(&self, id: &str) -> Result<Option<Workspace>, WorkspaceSetupError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
        let workspace = Self::find_workspace_in_transaction(&mut transaction, id).await?;
        transaction
            .commit()
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

        Ok(workspace)
    }

    async fn find_workspace_in_transaction(
        transaction: &mut SqliteTransaction<'_>,
        id: &str,
    ) -> Result<Option<Workspace>, WorkspaceSetupError> {
        let Some(row) = sqlx::query(
            r#"
            SELECT
                id,
                name,
                gpu_cloud_provider_id,
                lifecycle_state,
                environment_prepared_at
            FROM workspaces
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?
        else {
            return Ok(None);
        };

        decode_workspace(transaction, row).await.map(Some)
    }

    async fn list_workspaces_in_transaction(
        transaction: &mut SqliteTransaction<'_>,
    ) -> Result<WorkspaceCatalog, WorkspaceSetupError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                name,
                gpu_cloud_provider_id,
                lifecycle_state,
                environment_prepared_at
            FROM workspaces
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&mut **transaction)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

        let mut workspaces = Vec::with_capacity(rows.len());
        for row in rows {
            workspaces.push(decode_workspace(transaction, row).await?);
        }

        let catalog = WorkspaceCatalog { workspaces };
        workspace_validator::validate_workspace_catalog(&catalog)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogCorrupt)?;

        Ok(catalog)
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut transaction = self
                .pool
                .begin()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
            let catalog = Self::list_workspaces_in_transaction(&mut transaction).await?;
            transaction
                .commit()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

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
            let mut transaction = self
                .pool
                .begin()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            sqlx::query(
                r#"
                INSERT INTO workspaces (
                    id,
                    name,
                    gpu_cloud_provider_id,
                    lifecycle_state,
                    created_at,
                    updated_at,
                    environment_prepared_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&workspace.id)
            .bind(&workspace.name)
            .bind(gpu_cloud_provider_id_value(
                &workspace.gpu_cloud_provider_id,
            ))
            .bind(lifecycle_state_value(&workspace.lifecycle_state))
            .bind(&now)
            .bind(&now)
            .bind(&workspace.environment_prepared_at)
            .execute(&mut *transaction)
            .await
            .map_err(|error| {
                if is_unique_constraint(&error) {
                    WorkspaceSetupError::WorkspaceAlreadyExists
                } else {
                    WorkspaceSetupError::WorkspaceCatalogQueryFailed
                }
            })?;

            persist_workspace_details(&mut transaction, workspace).await?;
            transaction
                .commit()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

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

            let result = sqlx::query(
                r#"
                UPDATE workspaces
                SET
                    name = ?,
                    gpu_cloud_provider_id = ?,
                    lifecycle_state = ?,
                    updated_at = ?,
                    environment_prepared_at = ?
                WHERE id = ?
                "#,
            )
            .bind(&workspace.name)
            .bind(gpu_cloud_provider_id_value(
                &workspace.gpu_cloud_provider_id,
            ))
            .bind(lifecycle_state_value(&workspace.lifecycle_state))
            .bind(&now)
            .bind(&workspace.environment_prepared_at)
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

            delete_workspace_details(&mut transaction, &workspace.id).await?;
            persist_workspace_details(&mut transaction, workspace).await?;
            transaction
                .commit()
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;

            self.find_workspace(&workspace.id)
                .await?
                .ok_or(WorkspaceSetupError::WorkspaceCatalogQueryFailed)
        })
    }
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

#[cfg(test)]
mod test_fixtures {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use crate::{
        domain::{
            placement::PlacementPlan,
            provider_setup::GpuCloudProviderId,
            runtime::ResolvedRuntimeImageSnapshot,
            workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
            workspace::{
                PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot,
                ProviderResourceStatus, ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot,
                ServerlessEndpointSnapshot, Workspace, WorkspaceLifecycleState,
                WorkspaceProvisioningFailure, WorkspaceProvisioningFailureCode,
                WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
                WorkspaceProvisioningRecoveryAction,
            },
        },
        workspace_catalog::schema_bootstrap,
    };

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    pub(super) async fn bootstrapped_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        let mut transaction = pool
            .begin()
            .await
            .expect("begin schema bootstrap transaction");
        schema_bootstrap::bootstrap_and_check(&mut transaction)
            .await
            .expect("bootstrap schema");
        transaction
            .commit()
            .await
            .expect("commit schema bootstrap transaction");
        pool
    }

    pub(super) fn catalog_path(name: &str) -> std::path::PathBuf {
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
            endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
        }
    }

    pub(super) fn draft_workspace(id: &str, name: &str, workflow_preset_id: &str) -> Workspace {
        Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            id.to_string(),
            name.to_string(),
            placement_plan(workflow_preset_id),
            runtime_snapshot(),
        )
        .expect("valid draft workspace")
    }

    pub(super) fn volume(status: ProviderResourceStatus) -> PersistentStorageVolumeSnapshot {
        PersistentStorageVolumeSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "volume-id".to_string(),
            provider_resource_status: status,
            mount_path: "/workspace".to_string(),
        }
    }

    fn pod(status: ProviderResourceStatus) -> ProvisioningPodSnapshot {
        ProvisioningPodSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "pod-id".to_string(),
            provider_resource_status: status,
            provisioner_status_url: "https://worker.example/status".to_string(),
        }
    }

    fn endpoint(status: ProviderResourceStatus) -> ServerlessEndpointSnapshot {
        ServerlessEndpointSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "endpoint-id".to_string(),
            provider_resource_status: status,
            endpoint_invoke_url: "https://endpoint.example/run".to_string(),
        }
    }

    fn template(status: ProviderResourceStatus) -> RunPodEndpointTemplateSnapshot {
        RunPodEndpointTemplateSnapshot {
            template_id: "template-id".to_string(),
            provider_resource_status: status,
            endpoint_worker_image_ref: runtime_snapshot().endpoint_image_ref,
            mount_path: "/workspace".to_string(),
        }
    }

    fn failure() -> WorkspaceProvisioningFailure {
        WorkspaceProvisioningFailure {
            code: WorkspaceProvisioningFailureCode::ReadinessValidationFailed,
            phase: WorkspaceProvisioningPhase::ValidatingReadiness,
            source: WorkspaceProvisioningFailureSource::Native,
            recovery_action: WorkspaceProvisioningRecoveryAction::Retry,
        }
    }

    fn provisioning_workspace() -> Workspace {
        let mut workspace = draft_workspace("workspace-provisioning", "Provisioning", "preset-a");
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.active_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Running));
        workspace
    }

    pub(super) fn ready_workspace() -> Workspace {
        let mut workspace = draft_workspace("workspace-ready", "Ready", "preset-a");
        workspace.lifecycle_state = WorkspaceLifecycleState::Ready;
        workspace.persistent_storage_volume_snapshot = Some(volume(ProviderResourceStatus::Ready));
        workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template(ProviderResourceStatus::Ready)),
        });
        workspace.serverless_endpoint_snapshot = Some(endpoint(ProviderResourceStatus::Running));
        workspace.last_provisioning_pod_snapshot = Some(pod(ProviderResourceStatus::Terminated));
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());
        workspace
    }

    pub(super) fn provisioning_ready_and_failed_workspaces() -> [Workspace; 3] {
        let mut failed = provisioning_workspace();
        failed.id = "workspace-failed".to_string();
        failed.name = "Failed".to_string();
        failed.lifecycle_state = WorkspaceLifecycleState::Failed;
        failed.last_provisioning_failure = Some(failure());

        [provisioning_workspace(), ready_workspace(), failed]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::workspace::{Workspace, WorkspaceCatalog, WorkspaceLifecycleState},
        workspace_catalog::repository::WorkspaceCatalogRepository,
        workspace_setup::error::WorkspaceSetupError,
    };
    use sqlx::Row;
    use test_fixtures::{catalog_path, draft_workspace, volume};

    #[tokio::test]
    async fn connect_lists_empty_catalog() {
        let catalog = SqliteWorkspaceCatalog::connect(catalog_path("empty"))
            .await
            .expect("connect catalog");

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
        workspace.persistent_storage_volume_snapshot = Some(volume(
            crate::domain::workspace::ProviderResourceStatus::Ready,
        ));
        let updated = catalog
            .update_workspace(&workspace)
            .await
            .expect("update workspace");
        assert_eq!(updated, workspace);

        let row = sqlx::query(
            r#"
            SELECT
                workspaces.name,
                workspaces.lifecycle_state,
                workspace_runpod_placements.selected_workflow_preset_id
            FROM workspaces
            JOIN workspace_runpod_placements
                ON workspace_runpod_placements.workspace_id = workspaces.id
            WHERE workspaces.id = ?
            "#,
        )
        .bind("workspace-a")
        .fetch_one(&catalog.pool)
        .await
        .expect("read workspace row");
        let name: String = row.try_get("name").expect("name");
        let lifecycle_state: String = row.try_get("lifecycle_state").expect("lifecycle_state");
        let workflow_preset_id: String = row
            .try_get("selected_workflow_preset_id")
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
}
