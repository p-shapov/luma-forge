use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, IntoActiveModel, SqlErr,
    TransactionTrait,
};

use crate::{
    application::{
        runtimes::{
            ports::{RuntimePersistenceError, RuntimeTransitionRepository},
            RuntimeKind, RuntimeOperation, RuntimeOperationKind, RuntimeOperationState,
            RuntimeState,
        },
        workspace::Workspace,
    },
    infra::sqlite::entities::{runtime_operations, workspace_runtimes},
};

use super::{runtime_operation_repository, runtime_persistence_dispatcher};

pub struct SqliteRuntimeTransitionRepository {
    connection: DatabaseConnection,
}

impl SqliteRuntimeTransitionRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl RuntimeTransitionRepository for SqliteRuntimeTransitionRepository {
    #[diagnostic(show_error)]
    async fn save_transition(
        &self,
        #[diagnostic(show)] workspace: &Workspace,
        #[diagnostic(show)] operation: &RuntimeOperation,
    ) -> Result<(), RuntimePersistenceError> {
        if workspace.id != operation.workspace_id {
            return Err(RuntimePersistenceError::CorruptData);
        }
        runtime_persistence_dispatcher::validate_progress(operation)?;
        let transaction = self
            .connection
            .begin()
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)?;

        match &workspace.runtime {
            Some(runtime) if runtime.kind() == operation.runtime_kind => {
                upsert_anchor(
                    &workspace.id,
                    runtime.state,
                    operation.runtime_kind,
                    &transaction,
                )
                .await?;
                runtime_persistence_dispatcher::save_runtime(
                    &workspace.id,
                    &runtime.provider,
                    &transaction,
                )
                .await?;
            }
            Some(_) => return Err(RuntimePersistenceError::CorruptData),
            None if operation.kind == RuntimeOperationKind::Cleanup
                && operation.state == RuntimeOperationState::Succeeded =>
            {
                delete_anchor(&workspace.id, &transaction).await?;
            }
            None => return Err(RuntimePersistenceError::CorruptData),
        }
        save_operation(operation, &transaction).await?;
        runtime_persistence_dispatcher::save_progress(operation, &transaction).await?;
        transaction
            .commit()
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)
    }
}

async fn upsert_anchor(
    workspace_id: &str,
    state: RuntimeState,
    kind: RuntimeKind,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            let stored_kind =
                runtime_persistence_dispatcher::parse_runtime_kind(&model.runtime_kind)?;
            if stored_kind != kind {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.state = Set(runtime_state_value(state).to_owned());
            model
                .update(transaction)
                .await
                .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
        None => {
            workspace_runtimes::ActiveModel {
                workspace_id: Set(workspace_id.to_owned()),
                runtime_kind: Set(
                    runtime_persistence_dispatcher::runtime_kind_value(kind).to_owned()
                ),
                state: Set(runtime_state_value(state).to_owned()),
            }
            .insert(transaction)
            .await
            .map_err(|error| match error.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(_)) => {
                    RuntimePersistenceError::AlreadyExists
                }
                _ => RuntimePersistenceError::Unavailable,
            })?;
        }
    }
    Ok(())
}

async fn delete_anchor(
    workspace_id: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    let result = workspace_runtimes::Entity::delete_by_id(workspace_id)
        .exec(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?;
    if result.rows_affected == 0 {
        return Err(RuntimePersistenceError::NotFound);
    }
    Ok(())
}

async fn save_operation(
    operation: &RuntimeOperation,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            if model.workspace_id != operation.workspace_id
                || runtime_persistence_dispatcher::parse_runtime_kind(&model.runtime_kind)?
                    != operation.runtime_kind
                || model.operation_kind
                    != runtime_operation_repository::runtime_operation_kind_value(operation.kind)
            {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.running_workspace_id = Set(running_workspace_id(operation));
            model.state = Set(runtime_operation_repository::runtime_operation_state_value(
                operation.state,
            )
            .to_owned());
            model.updated_at = Set(operation.updated_at);
            model.finished_at = Set(operation.finished_at);
            model
                .update(transaction)
                .await
                .map_err(map_operation_error)?;
        }
        None => {
            runtime_operations::ActiveModel {
                id: Set(operation.id.to_string()),
                workspace_id: Set(operation.workspace_id.clone()),
                runtime_kind: Set(runtime_persistence_dispatcher::runtime_kind_value(
                    operation.runtime_kind,
                )
                .to_owned()),
                running_workspace_id: Set(running_workspace_id(operation)),
                operation_kind: Set(runtime_operation_repository::runtime_operation_kind_value(
                    operation.kind,
                )
                .to_owned()),
                state: Set(runtime_operation_repository::runtime_operation_state_value(
                    operation.state,
                )
                .to_owned()),
                trace_id: Set(operation.trace_id.map(|trace_id| trace_id.to_string())),
                created_at: Set(operation.created_at),
                updated_at: Set(operation.updated_at),
                finished_at: Set(operation.finished_at),
            }
            .insert(transaction)
            .await
            .map_err(map_operation_error)?;
        }
    }
    Ok(())
}

fn running_workspace_id(operation: &RuntimeOperation) -> Option<String> {
    (operation.state == RuntimeOperationState::Running).then(|| operation.workspace_id.clone())
}

fn runtime_state_value(state: RuntimeState) -> &'static str {
    match state {
        RuntimeState::Provisioning => "provisioning",
        RuntimeState::Ready => "ready",
        RuntimeState::CleaningUp => "cleaning_up",
        RuntimeState::Failed => "failed",
    }
}

fn map_operation_error(error: sea_orm::DbErr) -> RuntimePersistenceError {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => {
            RuntimePersistenceError::OperationAlreadyRunning
        }
        _ => RuntimePersistenceError::Unavailable,
    }
}
