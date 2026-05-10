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
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_setup::error::WorkspaceSetupError,
};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalog {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalog {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceSetupError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| WorkspaceSetupError::LocalStorageUnavailable)?;
        }

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .map_err(|_| WorkspaceSetupError::LocalStorageUnavailable)?;
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
            .map_err(|_| WorkspaceSetupError::LocalStorageUnavailable)?;
        let catalog = Self { pool };
        catalog.migrate().await?;
        Ok(catalog)
    }

    async fn migrate(&self) -> Result<(), WorkspaceSetupError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS workspaces (
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
        .execute(&self.pool)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspaces_lifecycle_state ON workspaces(lifecycle_state)",
        )
        .execute(&self.pool)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_workspaces_workflow_preset_id ON workspaces(workflow_preset_id)",
        )
        .execute(&self.pool)
        .await
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

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
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?
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
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

            let mut workspaces = Vec::with_capacity(rows.len());
            for row in rows {
                workspaces.push(decode_workspace_row(&row)?);
            }

            let catalog = WorkspaceCatalog { workspaces };
            workspace_validator::validate_workspace_catalog(&catalog)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

            Ok(catalog)
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            workspace_validator::validate_workspace(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
            if self.find_workspace(&workspace.id).await?.is_some() {
                return Err(WorkspaceSetupError::WorkspaceAlreadyExists);
            }

            let now = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
            let workspace_json = serde_json::to_string(workspace)
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
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
                    WorkspaceSetupError::WorkspaceCatalogUnavailable
                }
            })?;

            self.find_workspace(&workspace.id)
                .await?
                .ok_or(WorkspaceSetupError::WorkspaceCatalogUnavailable)
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

fn decode_workspace_row(row: &SqliteRow) -> Result<Workspace, WorkspaceSetupError> {
    let workspace_json: String = row
        .try_get("workspace_json")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let workspace: Workspace = serde_json::from_str(&workspace_json)
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

    workspace_validator::validate_workspace(&workspace)
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    validate_workspace_row(row, &workspace)?;

    Ok(workspace)
}

fn validate_workspace_row(
    row: &SqliteRow,
    workspace: &Workspace,
) -> Result<(), WorkspaceSetupError> {
    let id: String = row
        .try_get("id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let name: String = row
        .try_get("name")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let gpu_cloud_provider_id: String = row
        .try_get("gpu_cloud_provider_id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let lifecycle_state: String = row
        .try_get("lifecycle_state")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
    let workflow_preset_id: String = row
        .try_get("workflow_preset_id")
        .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

    if id != workspace.id
        || name != workspace.name
        || gpu_cloud_provider_id != gpu_cloud_provider_id_value(&workspace.gpu_cloud_provider_id)
        || lifecycle_state != lifecycle_state_value(&workspace.lifecycle_state)
        || workflow_preset_id != workspace.placement_plan.selected_workflow_preset().id
    {
        return Err(WorkspaceSetupError::WorkspaceCatalogUnavailable);
    }

    Ok(())
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
