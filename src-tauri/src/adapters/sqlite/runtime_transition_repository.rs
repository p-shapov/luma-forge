use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    EntityTrait, IntoActiveModel, QueryFilter, SqlErr, TransactionTrait,
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

use super::runtime_operation_repository;

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
        operation
            .validate_transition(workspace)
            .map_err(|_| RuntimePersistenceError::CorruptData)?;
        let provider_payload = workspace
            .runtime
            .as_ref()
            .map(|runtime| serde_json::to_string(&runtime.provider))
            .transpose()
            .map_err(|_| RuntimePersistenceError::CorruptData)?;
        let progress_payload = serde_json::to_string(&operation.progress)
            .map_err(|_| RuntimePersistenceError::CorruptData)?;

        let transaction = self
            .connection
            .begin()
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)?;
        let operation_is_new = save_operation(operation, &progress_payload, &transaction).await?;

        match &workspace.runtime {
            Some(runtime) => {
                save_anchor(
                    &workspace.id,
                    runtime.state,
                    provider_payload
                        .as_deref()
                        .ok_or(RuntimePersistenceError::CorruptData)?,
                    operation,
                    operation_is_new,
                    &transaction,
                )
                .await?;
            }
            None => delete_anchor(&workspace.id, &transaction).await?,
        }

        transaction
            .commit()
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)
    }
}

async fn save_anchor(
    workspace_id: &str,
    state: RuntimeState,
    provider_payload: &str,
    operation: &RuntimeOperation,
    operation_is_new: bool,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    if operation_is_new && operation.state == RuntimeOperationState::Running {
        return match operation.kind {
            RuntimeOperationKind::Provision => insert_anchor(
                workspace_id,
                state,
                operation.runtime_kind,
                provider_payload,
                transaction,
            )
            .await
            .map_err(|error| match error {
                RuntimePersistenceError::AlreadyExists => {
                    RuntimePersistenceError::OperationAlreadyRunning
                }
                error => error,
            }),
            RuntimeOperationKind::Cleanup => {
                claim_cleanup_anchor(
                    workspace_id,
                    operation.runtime_kind,
                    provider_payload,
                    transaction,
                )
                .await
            }
        };
    }
    upsert_anchor(
        workspace_id,
        state,
        operation.runtime_kind,
        provider_payload,
        transaction,
    )
    .await
}

async fn upsert_anchor(
    workspace_id: &str,
    state: RuntimeState,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            if model
                .runtime_kind
                .parse::<RuntimeKind>()
                .map_err(|_| RuntimePersistenceError::CorruptData)?
                != kind
            {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.state = Set(runtime_state_value(state).to_owned());
            model.provider_payload = Set(provider_payload.to_owned());
            model
                .update(transaction)
                .await
                .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
        None => {
            insert_anchor(workspace_id, state, kind, provider_payload, transaction).await?;
        }
    }
    Ok(())
}

async fn insert_anchor(
    workspace_id: &str,
    state: RuntimeState,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    workspace_runtimes::ActiveModel {
        workspace_id: Set(workspace_id.to_owned()),
        runtime_kind: Set(kind.as_str().to_owned()),
        state: Set(runtime_state_value(state).to_owned()),
        provider_payload: Set(provider_payload.to_owned()),
    }
    .insert(transaction)
    .await
    .map_err(|error| match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => RuntimePersistenceError::AlreadyExists,
        _ => RuntimePersistenceError::Unavailable,
    })?;
    Ok(())
}

async fn claim_cleanup_anchor(
    workspace_id: &str,
    kind: RuntimeKind,
    provider_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    let result = workspace_runtimes::Entity::update_many()
        .col_expr(
            workspace_runtimes::Column::State,
            Expr::value("cleaning_up"),
        )
        .col_expr(
            workspace_runtimes::Column::ProviderPayload,
            Expr::value(provider_payload.to_owned()),
        )
        .filter(workspace_runtimes::Column::WorkspaceId.eq(workspace_id))
        .filter(workspace_runtimes::Column::RuntimeKind.eq(kind.as_str()))
        .filter(workspace_runtimes::Column::State.is_in(["ready", "failed"]))
        .exec(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?;
    if result.rows_affected == 1 {
        return Ok(());
    }

    match workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        None => Err(RuntimePersistenceError::NotFound),
        Some(model) if model.runtime_kind != kind.as_str() => {
            Err(RuntimePersistenceError::CorruptData)
        }
        Some(model) if matches!(model.state.as_str(), "provisioning" | "cleaning_up") => {
            Err(RuntimePersistenceError::OperationAlreadyRunning)
        }
        Some(_) => Err(RuntimePersistenceError::CorruptData),
    }
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
    progress_payload: &str,
    transaction: &sea_orm::DatabaseTransaction,
) -> Result<bool, RuntimePersistenceError> {
    match runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            if model.workspace_id != operation.workspace_id
                || model
                    .runtime_kind
                    .parse::<RuntimeKind>()
                    .map_err(|_| RuntimePersistenceError::CorruptData)?
                    != operation.runtime_kind
                || model.operation_kind
                    != runtime_operation_repository::runtime_operation_kind_value(operation.kind)
            {
                return Err(RuntimePersistenceError::CorruptData);
            }
            let mut model = model.into_active_model();
            model.state = Set(runtime_operation_repository::runtime_operation_state_value(
                operation.state,
            )
            .to_owned());
            model.progress_payload = Set(progress_payload.to_owned());
            model.updated_at = Set(operation.updated_at);
            model.finished_at = Set(operation.finished_at);
            model
                .update(transaction)
                .await
                .map_err(map_operation_error)?;
            Ok(false)
        }
        None => {
            runtime_operations::ActiveModel {
                id: Set(operation.id.to_string()),
                workspace_id: Set(operation.workspace_id.clone()),
                runtime_kind: Set(operation.runtime_kind.as_str().to_owned()),
                operation_kind: Set(runtime_operation_repository::runtime_operation_kind_value(
                    operation.kind,
                )
                .to_owned()),
                state: Set(runtime_operation_repository::runtime_operation_state_value(
                    operation.state,
                )
                .to_owned()),
                trace_id: Set(operation.trace_id.map(|trace_id| trace_id.to_string())),
                progress_payload: Set(progress_payload.to_owned()),
                created_at: Set(operation.created_at),
                updated_at: Set(operation.updated_at),
                finished_at: Set(operation.finished_at),
            }
            .insert(transaction)
            .await
            .map_err(map_operation_error)?;
            Ok(true)
        }
    }
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
