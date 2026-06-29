use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};

use crate::infra::sqlite::{
    entities::{runpod_workspace_runtimes, workspaces},
    errors::SqliteInfraError,
};

pub struct SqliteWorkspaceRepository<'db> {
    connection: &'db DatabaseConnection,
}

impl<'db> SqliteWorkspaceRepository<'db> {
    pub fn new(connection: &'db DatabaseConnection) -> Self {
        Self { connection }
    }

    pub async fn list_workspaces(&self) -> Result<Vec<workspaces::Model>, SqliteInfraError> {
        let rows = workspaces::Entity::find()
            .order_by_asc(workspaces::COLUMN.created_at)
            .all(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("list workspaces"))?;

        Ok(rows)
    }

    pub async fn find_workspace(
        &self,
        id: &str,
    ) -> Result<Option<workspaces::Model>, SqliteInfraError> {
        let row = workspaces::Entity::find_by_id(id)
            .one(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("find workspace"))?;

        Ok(row)
    }

    pub async fn insert_workspace(
        &self,
        workspace: workspaces::Model,
    ) -> Result<(), SqliteInfraError> {
        workspace
            .into_active_model()
            .insert(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("insert workspace"))?;

        Ok(())
    }

    pub async fn update_workspace(
        &self,
        workspace: workspaces::Model,
    ) -> Result<(), SqliteInfraError> {
        workspace
            .into_active_model()
            .update(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("update workspace"))?;

        Ok(())
    }

    pub async fn find_runpod_runtime(
        &self,
        workspace_id: &str,
    ) -> Result<Option<runpod_workspace_runtimes::Model>, SqliteInfraError> {
        let row = runpod_workspace_runtimes::Entity::find()
            .filter(
                runpod_workspace_runtimes::COLUMN
                    .workspace_id
                    .eq(workspace_id),
            )
            .one(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("find runpod runtime"))?;

        Ok(row)
    }

    pub async fn insert_runpod_runtime(
        &self,
        runtime: runpod_workspace_runtimes::Model,
    ) -> Result<(), SqliteInfraError> {
        runtime
            .into_active_model()
            .insert(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("insert runpod runtime"))?;

        Ok(())
    }

    pub async fn update_runpod_runtime(
        &self,
        runtime: runpod_workspace_runtimes::Model,
    ) -> Result<(), SqliteInfraError> {
        runtime
            .into_active_model()
            .update(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("update runpod runtime"))?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<(), SqliteInfraError> {
        workspaces::Entity::delete_by_id(id)
            .exec(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed("delete workspace"))?;

        Ok(())
    }
}
