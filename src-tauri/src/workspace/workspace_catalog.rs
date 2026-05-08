use std::{future::Future, path::Path, pin::Pin};

use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

use crate::{
    domain::workspace::WorkspaceLifecycleState,
    workspace::{
        workspace_contracts::{Workspace, WorkspaceCatalog},
        workspace_setup::WorkspaceSetupError,
    },
};

pub trait WorkspaceCatalogRepository: Send + Sync {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>;

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>;
}

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalog {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableWorkspaceCatalog;

impl WorkspaceCatalogRepository for UnavailableWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }

    fn insert_workspace<'a>(
        &'a self,
        _workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
    }
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
        let Some(row) = sqlx::query("SELECT workspace_json FROM workspaces WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?
        else {
            return Ok(None);
        };

        let workspace_json: String = row
            .try_get("workspace_json")
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
        serde_json::from_str(&workspace_json)
            .map(Some)
            .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            let rows = sqlx::query("SELECT workspace_json FROM workspaces ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;

            let mut workspaces = Vec::with_capacity(rows.len());
            for row in rows {
                let workspace_json: String = row
                    .try_get("workspace_json")
                    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
                let workspace = serde_json::from_str(&workspace_json)
                    .map_err(|_| WorkspaceSetupError::WorkspaceCatalogUnavailable)?;
                workspaces.push(workspace);
            }

            Ok(WorkspaceCatalog { workspaces })
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
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
            .bind("runpod")
            .bind(lifecycle_state)
            .bind(&workspace.placement_plan.selected_workflow_preset.id)
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

fn lifecycle_state_value(lifecycle_state: &WorkspaceLifecycleState) -> &'static str {
    match lifecycle_state {
        WorkspaceLifecycleState::Draft => "draft",
        WorkspaceLifecycleState::Provisioning => "provisioning",
        WorkspaceLifecycleState::Ready => "ready",
        WorkspaceLifecycleState::Failed => "failed",
    }
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database_error) if database_error.is_unique_violation())
}

#[cfg(test)]
#[path = "workspace_catalog_tests.rs"]
mod workspace_catalog_tests;
