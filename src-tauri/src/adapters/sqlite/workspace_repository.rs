use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, SqlErr};

use crate::{
    application::{
        runtimes::{CatalogRef, Runtime, RuntimeKind, RuntimeProvider, RuntimeState},
        workspace::{
            ports::{WorkspaceRepository, WorkspaceRepositoryError},
            Workspace,
        },
    },
    infra::sqlite::entities::{workspace_runtimes, workspaces},
};

use super::runtime_persistence_dispatcher;

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
        let Some((workspace, anchor)) = workspaces::Entity::find_by_id(id)
            .find_also_related(workspace_runtimes::Entity)
            .one(&self.connection)
            .await
            .map_err(|_| WorkspaceRepositoryError::Unavailable)?
        else {
            return Ok(None);
        };
        let runtime = match anchor {
            Some(anchor) => {
                let kind = parse_runtime_kind(&anchor.runtime_kind)?;
                let provider = runtime_persistence_dispatcher::load_runtime(
                    &anchor.workspace_id,
                    kind,
                    &self.connection,
                )
                .await
                .map_err(map_runtime_error)?;
                Some(map_runtime(anchor, provider)?)
            }
            None => None,
        };
        Ok(Some(map_workspace(workspace, runtime)))
    }

    #[diagnostic(show_output, show_error)]
    async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
        let rows = workspaces::Entity::find()
            .find_also_related(workspace_runtimes::Entity)
            .all(&self.connection)
            .await
            .map_err(|_| WorkspaceRepositoryError::Unavailable)?;
        let runtime_ids = rows
            .iter()
            .filter_map(|(_, anchor)| anchor.as_ref())
            .map(|anchor| {
                parse_runtime_kind(&anchor.runtime_kind)
                    .map(|kind| (anchor.workspace_id.clone(), kind))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut providers =
            runtime_persistence_dispatcher::load_runtimes(&runtime_ids, &self.connection)
                .await
                .map_err(map_runtime_error)?;

        rows.into_iter()
            .map(|(workspace, anchor)| {
                let runtime = anchor
                    .map(|anchor| {
                        let provider = providers
                            .remove(&anchor.workspace_id)
                            .ok_or(WorkspaceRepositoryError::CorruptData)?;
                        map_runtime(anchor, provider)
                    })
                    .transpose()?;
                Ok(map_workspace(workspace, runtime))
            })
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

fn map_workspace(model: workspaces::Model, runtime: Option<Runtime>) -> Workspace {
    Workspace {
        id: model.id,
        workflow: CatalogRef::new(model.workflow_id, model.workflow_revision),
        created_at: model.created_at,
        runtime,
    }
}

fn map_runtime(
    anchor: workspace_runtimes::Model,
    provider: RuntimeProvider,
) -> Result<Runtime, WorkspaceRepositoryError> {
    Ok(Runtime {
        state: parse_runtime_state(&anchor.state)?,
        provider,
    })
}

fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, WorkspaceRepositoryError> {
    runtime_persistence_dispatcher::parse_runtime_kind(value).map_err(map_runtime_error)
}

fn parse_runtime_state(value: &str) -> Result<RuntimeState, WorkspaceRepositoryError> {
    match value {
        "provisioning" => Ok(RuntimeState::Provisioning),
        "ready" => Ok(RuntimeState::Ready),
        "cleaning_up" => Ok(RuntimeState::CleaningUp),
        "failed" => Ok(RuntimeState::Failed),
        _ => Err(WorkspaceRepositoryError::CorruptData),
    }
}

fn map_runtime_error(
    error: crate::application::runtimes::ports::RuntimePersistenceError,
) -> WorkspaceRepositoryError {
    match error {
        crate::application::runtimes::ports::RuntimePersistenceError::Unavailable => {
            WorkspaceRepositoryError::Unavailable
        }
        _ => WorkspaceRepositoryError::CorruptData,
    }
}
