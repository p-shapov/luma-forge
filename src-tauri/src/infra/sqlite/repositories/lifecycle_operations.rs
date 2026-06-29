use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
};

use crate::infra::sqlite::{
    entities::{lifecycle_operations, runpod_operation_payloads},
    errors::SqliteInfraError,
};

#[derive(Debug, Clone, Default)]
pub struct LifecycleOperationFilter {
    pub workspace_id: Option<String>,
    pub states: Vec<String>,
}

pub struct SqliteLifecycleOperationRepository<'db> {
    connection: &'db DatabaseConnection,
}

impl<'db> SqliteLifecycleOperationRepository<'db> {
    pub fn new(connection: &'db DatabaseConnection) -> Self {
        Self { connection }
    }

    pub async fn insert_operation(
        &self,
        operation: lifecycle_operations::Model,
    ) -> Result<(), SqliteInfraError> {
        operation
            .into_active_model()
            .insert(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "insert lifecycle operation",
            ))?;

        Ok(())
    }

    pub async fn insert_runpod_payload(
        &self,
        payload: runpod_operation_payloads::Model,
    ) -> Result<(), SqliteInfraError> {
        payload
            .into_active_model()
            .insert(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "insert runpod operation payload",
            ))?;

        Ok(())
    }

    pub async fn find_operation(
        &self,
        id: &str,
    ) -> Result<Option<lifecycle_operations::Model>, SqliteInfraError> {
        let row = lifecycle_operations::Entity::find_by_id(id)
            .one(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "find lifecycle operation",
            ))?;

        Ok(row)
    }

    pub async fn list_operations(
        &self,
        filter: Option<LifecycleOperationFilter>,
    ) -> Result<Vec<lifecycle_operations::Model>, SqliteInfraError> {
        let mut query = lifecycle_operations::Entity::find();

        if let Some(filter) = filter {
            let LifecycleOperationFilter {
                workspace_id,
                states,
            } = filter;

            if let Some(workspace_id) = workspace_id {
                query = query.filter(lifecycle_operations::COLUMN.workspace_id.eq(workspace_id));
            }

            if !states.is_empty() {
                query = query.filter(lifecycle_operations::COLUMN.state.is_in(states));
            }
        }

        let rows = query
            .order_by_asc(lifecycle_operations::COLUMN.created_at)
            .all(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "list lifecycle operations",
            ))?;

        Ok(rows)
    }

    pub async fn latest_operation(
        &self,
        workspace_id: &str,
    ) -> Result<Option<lifecycle_operations::Model>, SqliteInfraError> {
        let row = lifecycle_operations::Entity::find()
            .filter(lifecycle_operations::COLUMN.workspace_id.eq(workspace_id))
            .order_by_desc(lifecycle_operations::COLUMN.created_at)
            .order_by_desc(lifecycle_operations::COLUMN.updated_at)
            .order_by_desc(lifecycle_operations::COLUMN.id)
            .one(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "latest lifecycle operation",
            ))?;

        Ok(row)
    }

    pub async fn update_operation(
        &self,
        operation: lifecycle_operations::Model,
    ) -> Result<(), SqliteInfraError> {
        operation
            .into_active_model()
            .update(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "update lifecycle operation",
            ))?;

        Ok(())
    }

    pub async fn find_runpod_payload(
        &self,
        operation_id: &str,
    ) -> Result<Option<runpod_operation_payloads::Model>, SqliteInfraError> {
        let row = runpod_operation_payloads::Entity::find_by_id(operation_id)
            .one(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "find runpod operation payload",
            ))?;

        Ok(row)
    }

    pub async fn update_runpod_payload(
        &self,
        payload: runpod_operation_payloads::Model,
    ) -> Result<(), SqliteInfraError> {
        payload
            .into_active_model()
            .update(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "update runpod operation payload",
            ))?;

        Ok(())
    }

    pub async fn delete_for_workspace(&self, workspace_id: &str) -> Result<(), SqliteInfraError> {
        lifecycle_operations::Entity::delete_many()
            .filter(lifecycle_operations::COLUMN.workspace_id.eq(workspace_id))
            .exec(self.connection)
            .await
            .map_err(SqliteInfraError::statement_failed(
                "delete lifecycle operations",
            ))?;

        Ok(())
    }
}
