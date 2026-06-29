use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, Order, QueryFilter, QueryOrder,
    Set,
};

use crate::infra::sqlite::{
    entities::{runpod_workspace_runtimes, workspaces},
    errors::SqliteInfraError,
    model::{format_timestamp, parse_timestamp, PersistedRunpodRuntime, PersistedWorkspace},
};

pub struct SqliteWorkspaceRepository<'db, C: ConnectionTrait> {
    connection: &'db C,
}

impl<'db, C: ConnectionTrait> SqliteWorkspaceRepository<'db, C> {
    pub fn new(connection: &'db C) -> Self {
        Self { connection }
    }

    pub async fn list_workspaces(&self) -> Result<Vec<PersistedWorkspace>, SqliteInfraError> {
        let rows = workspaces::Entity::find()
            .order_by(workspaces::Column::CreatedAt, Order::Asc)
            .all(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "list workspaces",
                message: error.to_string(),
            })?;

        rows.into_iter().map(workspace_from_model).collect()
    }

    pub async fn find_workspace(
        &self,
        id: &str,
    ) -> Result<Option<PersistedWorkspace>, SqliteInfraError> {
        let row = workspaces::Entity::find_by_id(id)
            .one(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "find workspace",
                message: error.to_string(),
            })?;

        row.map(workspace_from_model).transpose()
    }

    pub async fn insert_workspace(
        &self,
        workspace: PersistedWorkspace,
    ) -> Result<(), SqliteInfraError> {
        workspace_active_model(workspace, "insert workspace")?
            .insert(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "insert workspace",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn update_workspace(
        &self,
        workspace: PersistedWorkspace,
    ) -> Result<(), SqliteInfraError> {
        workspace_active_model(workspace, "update workspace")?
            .update(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "update workspace",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn find_runpod_runtime(
        &self,
        workspace_id: &str,
    ) -> Result<Option<PersistedRunpodRuntime>, SqliteInfraError> {
        let row = runpod_workspace_runtimes::Entity::find()
            .filter(runpod_workspace_runtimes::Column::WorkspaceId.eq(workspace_id))
            .one(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "find runpod runtime",
                message: error.to_string(),
            })?;

        Ok(row.map(runtime_from_model))
    }

    pub async fn insert_runpod_runtime(
        &self,
        runtime: PersistedRunpodRuntime,
    ) -> Result<(), SqliteInfraError> {
        runtime_active_model(runtime)
            .insert(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "insert runpod runtime",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn update_runpod_runtime(
        &self,
        runtime: PersistedRunpodRuntime,
    ) -> Result<(), SqliteInfraError> {
        runtime_active_model(runtime)
            .update(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "update runpod runtime",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), SqliteInfraError> {
        workspaces::Entity::delete_by_id(id)
            .exec(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "delete workspace",
                message: error.to_string(),
            })?;

        Ok(())
    }
}

fn workspace_from_model(row: workspaces::Model) -> Result<PersistedWorkspace, SqliteInfraError> {
    Ok(PersistedWorkspace {
        id: row.id,
        workflow_id: row.workflow_id,
        workflow_version: row.workflow_version,
        state: row.state,
        runtime_kind: row.runtime_kind,
        created_at: parse_timestamp(&row.created_at, "read workspace", "created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "read workspace", "updated_at")?,
    })
}

fn workspace_active_model(
    workspace: PersistedWorkspace,
    operation: &'static str,
) -> Result<workspaces::ActiveModel, SqliteInfraError> {
    Ok(workspaces::ActiveModel {
        id: Set(workspace.id),
        workflow_id: Set(workspace.workflow_id),
        workflow_version: Set(workspace.workflow_version),
        state: Set(workspace.state),
        runtime_kind: Set(workspace.runtime_kind),
        created_at: Set(format_timestamp(
            workspace.created_at,
            operation,
            "created_at",
        )?),
        updated_at: Set(format_timestamp(
            workspace.updated_at,
            operation,
            "updated_at",
        )?),
    })
}

fn runtime_from_model(row: runpod_workspace_runtimes::Model) -> PersistedRunpodRuntime {
    PersistedRunpodRuntime {
        workspace_id: row.workspace_id,
        datacenter_id: row.datacenter_id,
        gpu_id: row.gpu_id,
        volume_size_gb: row.volume_size_gb,
        network_volume_id: row.network_volume_id,
        provisioner_pod_id: row.provisioner_pod_id,
        endpoint_id: row.endpoint_id,
        template_id: row.template_id,
    }
}

fn runtime_active_model(runtime: PersistedRunpodRuntime) -> runpod_workspace_runtimes::ActiveModel {
    runpod_workspace_runtimes::ActiveModel {
        workspace_id: Set(runtime.workspace_id),
        datacenter_id: Set(runtime.datacenter_id),
        gpu_id: Set(runtime.gpu_id),
        volume_size_gb: Set(runtime.volume_size_gb),
        network_volume_id: Set(runtime.network_volume_id),
        provisioner_pod_id: Set(runtime.provisioner_pod_id),
        endpoint_id: Set(runtime.endpoint_id),
        template_id: Set(runtime.template_id),
    }
}
