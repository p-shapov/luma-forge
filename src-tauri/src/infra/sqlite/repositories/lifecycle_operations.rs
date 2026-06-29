use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, Order, QueryFilter, QueryOrder,
    Set,
};

use crate::infra::sqlite::{
    entities::{lifecycle_operations, runpod_operation_payloads},
    errors::SqliteInfraError,
    model::{
        format_timestamp, parse_timestamp, PersistedLifecycleOperation,
        PersistedLifecycleOperationFilter, PersistedRunpodPayload,
    },
};

pub struct SqliteLifecycleOperationRepository<'db, C: ConnectionTrait> {
    connection: &'db C,
}

impl<'db, C: ConnectionTrait> SqliteLifecycleOperationRepository<'db, C> {
    pub fn new(connection: &'db C) -> Self {
        Self { connection }
    }

    pub async fn insert_operation(
        &self,
        operation: PersistedLifecycleOperation,
    ) -> Result<(), SqliteInfraError> {
        operation_active_model(operation, "insert lifecycle operation")?
            .insert(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "insert lifecycle operation",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn insert_runpod_payload(
        &self,
        payload: PersistedRunpodPayload,
    ) -> Result<(), SqliteInfraError> {
        payload_active_model(payload)
            .insert(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "insert runpod operation payload",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn find_operation(
        &self,
        id: &str,
    ) -> Result<Option<PersistedLifecycleOperation>, SqliteInfraError> {
        let row = lifecycle_operations::Entity::find_by_id(id)
            .one(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "find lifecycle operation",
                message: error.to_string(),
            })?;

        row.map(operation_from_model).transpose()
    }

    pub async fn list_operations(
        &self,
        filter: Option<PersistedLifecycleOperationFilter>,
    ) -> Result<Vec<PersistedLifecycleOperation>, SqliteInfraError> {
        let mut query = lifecycle_operations::Entity::find();

        if let Some(filter) = filter {
            let PersistedLifecycleOperationFilter {
                workspace_id,
                states,
            } = filter;

            if let Some(workspace_id) = workspace_id {
                query = query.filter(lifecycle_operations::Column::WorkspaceId.eq(workspace_id));
            }

            if !states.is_empty() {
                query = query.filter(lifecycle_operations::Column::State.is_in(states));
            }
        }

        let rows = query
            .order_by(lifecycle_operations::Column::CreatedAt, Order::Asc)
            .all(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "list lifecycle operations",
                message: error.to_string(),
            })?;

        rows.into_iter().map(operation_from_model).collect()
    }

    pub async fn latest_operation(
        &self,
        workspace_id: &str,
    ) -> Result<Option<PersistedLifecycleOperation>, SqliteInfraError> {
        let row = lifecycle_operations::Entity::find()
            .filter(lifecycle_operations::Column::WorkspaceId.eq(workspace_id))
            .order_by(lifecycle_operations::Column::CreatedAt, Order::Desc)
            .order_by(lifecycle_operations::Column::UpdatedAt, Order::Desc)
            .order_by(lifecycle_operations::Column::Id, Order::Desc)
            .one(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "latest lifecycle operation",
                message: error.to_string(),
            })?;

        row.map(operation_from_model).transpose()
    }

    pub async fn update_operation(
        &self,
        operation: PersistedLifecycleOperation,
    ) -> Result<(), SqliteInfraError> {
        operation_active_model(operation, "update lifecycle operation")?
            .update(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "update lifecycle operation",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn find_runpod_payload(
        &self,
        operation_id: &str,
    ) -> Result<Option<PersistedRunpodPayload>, SqliteInfraError> {
        let row = runpod_operation_payloads::Entity::find_by_id(operation_id)
            .one(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "find runpod operation payload",
                message: error.to_string(),
            })?;

        Ok(row.map(payload_from_model))
    }

    pub async fn update_runpod_payload(
        &self,
        payload: PersistedRunpodPayload,
    ) -> Result<(), SqliteInfraError> {
        payload_active_model(payload)
            .update(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "update runpod operation payload",
                message: error.to_string(),
            })?;

        Ok(())
    }

    pub async fn delete_for_workspace(&self, workspace_id: &str) -> Result<(), SqliteInfraError> {
        lifecycle_operations::Entity::delete_many()
            .filter(lifecycle_operations::Column::WorkspaceId.eq(workspace_id))
            .exec(self.connection)
            .await
            .map_err(|error| SqliteInfraError::StatementFailed {
                operation: "delete lifecycle operations",
                message: error.to_string(),
            })?;

        Ok(())
    }
}

fn operation_from_model(
    row: lifecycle_operations::Model,
) -> Result<PersistedLifecycleOperation, SqliteInfraError> {
    Ok(PersistedLifecycleOperation {
        id: row.id,
        workspace_id: row.workspace_id,
        operation_kind: row.operation_kind,
        state: row.state,
        created_at: parse_timestamp(&row.created_at, "read lifecycle operation", "created_at")?,
        updated_at: parse_timestamp(&row.updated_at, "read lifecycle operation", "updated_at")?,
        finished_at: row
            .finished_at
            .as_deref()
            .map(|finished_at| {
                parse_timestamp(finished_at, "read lifecycle operation", "finished_at")
            })
            .transpose()?,
    })
}

fn operation_active_model(
    operation: PersistedLifecycleOperation,
    action: &'static str,
) -> Result<lifecycle_operations::ActiveModel, SqliteInfraError> {
    Ok(lifecycle_operations::ActiveModel {
        id: Set(operation.id),
        workspace_id: Set(operation.workspace_id),
        operation_kind: Set(operation.operation_kind),
        state: Set(operation.state),
        created_at: Set(format_timestamp(
            operation.created_at,
            action,
            "created_at",
        )?),
        updated_at: Set(format_timestamp(
            operation.updated_at,
            action,
            "updated_at",
        )?),
        finished_at: Set(operation
            .finished_at
            .map(|finished_at| format_timestamp(finished_at, action, "finished_at"))
            .transpose()?),
    })
}

fn payload_from_model(row: runpod_operation_payloads::Model) -> PersistedRunpodPayload {
    PersistedRunpodPayload {
        operation_id: row.operation_id,
        step: row.step,
    }
}

fn payload_active_model(payload: PersistedRunpodPayload) -> runpod_operation_payloads::ActiveModel {
    runpod_operation_payloads::ActiveModel {
        operation_id: Set(payload.operation_id),
        step: Set(payload.step),
    }
}
