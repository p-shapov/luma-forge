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

#[cfg(test)]
use migrations::{CURRENT_PERSISTENCE_VERSION, PERSISTENCE_VERSION_KEY};

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

    #[cfg(test)]
    async fn in_memory() -> Result<Self, WorkspaceSetupError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
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
#[path = "tests.rs"]
mod tests;
