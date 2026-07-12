use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, SqlErr};

use crate::{
    application::{
        runtimes::{CatalogRef, RuntimeKind},
        workspace::{
            ports::{WorkspaceRepository, WorkspaceRepositoryError},
            Workspace,
        },
    },
    infra::sqlite::entities::{workspace_runtimes, workspaces},
};

pub struct SqliteWorkspaceRepository {
    connection: DatabaseConnection,
}

impl SqliteWorkspaceRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl WorkspaceRepository for SqliteWorkspaceRepository {
    #[diagnostic(show_output, show_error)]
    async fn create(
        &self,
        #[diagnostic(show)] workspace: Workspace,
    ) -> Result<Workspace, WorkspaceRepositoryError> {
        workspaces::ActiveModel {
            id: Set(workspace.id.clone()),
            workflow_id: Set(workspace.workflow.id.clone()),
            workflow_revision: Set(workspace.workflow.revision.clone()),
            created_at: Set(workspace.created_at),
        }
        .insert(&self.connection)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => WorkspaceRepositoryError::AlreadyExists,
            _ => WorkspaceRepositoryError::Unavailable,
        })?;
        Ok(workspace)
    }

    #[diagnostic(show_output, show_error)]
    async fn get(
        &self,
        #[diagnostic(show)] id: &str,
    ) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
        let Some((workspace, runtime)) = workspaces::Entity::find_by_id(id)
            .find_also_related(workspace_runtimes::Entity)
            .one(&self.connection)
            .await
            .map_err(|_| WorkspaceRepositoryError::Unavailable)?
        else {
            return Ok(None);
        };
        map_workspace(workspace, runtime).map(Some)
    }

    #[diagnostic(show_output, show_error)]
    async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
        let rows = workspaces::Entity::find()
            .find_also_related(workspace_runtimes::Entity)
            .all(&self.connection)
            .await
            .map_err(|_| WorkspaceRepositoryError::Unavailable)?;
        rows.into_iter()
            .map(|(workspace, runtime)| map_workspace(workspace, runtime))
            .collect()
    }

    #[diagnostic(show_output, show_error)]
    async fn delete(&self, #[diagnostic(show)] id: &str) -> Result<bool, WorkspaceRepositoryError> {
        Ok(workspaces::Entity::delete_by_id(id)
            .exec(&self.connection)
            .await
            .map_err(|_| WorkspaceRepositoryError::Unavailable)?
            .rows_affected
            > 0)
    }
}

fn map_workspace(
    model: workspaces::Model,
    runtime: Option<workspace_runtimes::Model>,
) -> Result<Workspace, WorkspaceRepositoryError> {
    let runtime = runtime
        .map(|runtime| match runtime.provider_kind.as_str() {
            "runpod" => Ok(RuntimeKind::Runpod),
            _ => Err(WorkspaceRepositoryError::CorruptData),
        })
        .transpose()?;
    Ok(Workspace {
        id: model.id,
        workflow: CatalogRef::new(model.workflow_id, model.workflow_revision),
        created_at: model.created_at,
        runtime,
    })
}
