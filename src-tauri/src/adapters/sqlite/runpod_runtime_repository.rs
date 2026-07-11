use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, IntoActiveModel, SqlErr,
    TransactionTrait,
};

use crate::{
    application::{
        lifecycle::{LifecycleOperation, LifecycleOperationState},
        runtimes::{
            ports::{RuntimeTransitionRepository, RuntimeTransitionRepositoryError},
            runpod::{
                RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeRepository,
                RunpodRuntimeRepositoryError, RunpodRuntimeResources, RunpodRuntimeState,
            },
        },
    },
    infra::sqlite::entities::{
        lifecycle_operations, runpod_lifecycle_progress, runpod_workspace_runtimes,
        workspace_runtimes,
    },
};

use super::lifecycle_operation_repository::{
    operation_kind_value, operation_state_value, progress_value,
};

pub struct SqliteRunpodRuntimeRepository {
    connection: DatabaseConnection,
}

impl SqliteRunpodRuntimeRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }
}

#[async_trait::async_trait]
impl RunpodRuntimeRepository for SqliteRunpodRuntimeRepository {
    async fn get(
        &self,
        workspace_id: &str,
    ) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError> {
        let Some((anchor, model)) = workspace_runtimes::Entity::find_by_id(workspace_id)
            .find_also_related(runpod_workspace_runtimes::Entity)
            .one(&self.connection)
            .await
            .map_err(|_| RunpodRuntimeRepositoryError::Unavailable)?
        else {
            return Ok(None);
        };
        if anchor.provider_kind != "runpod" {
            return Ok(None);
        }
        let model = model.ok_or(RunpodRuntimeRepositoryError::CorruptData)?;
        map_runtime(model).map(Some)
    }
}

#[async_trait::async_trait]
impl RuntimeTransitionRepository<RunpodRuntime> for SqliteRunpodRuntimeRepository {
    async fn save_transition(
        &self,
        runtime: &RunpodRuntime,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        if runtime.workspace_id != operation.workspace_id {
            return Err(RuntimeTransitionRepositoryError::CorruptData);
        }

        let transaction = self
            .connection
            .begin()
            .await
            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
        let anchor = workspace_runtimes::Entity::find_by_id(&runtime.workspace_id)
            .one(&transaction)
            .await
            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
        if anchor
            .as_ref()
            .is_some_and(|anchor| anchor.provider_kind != "runpod")
        {
            return Err(RuntimeTransitionRepositoryError::CorruptData);
        }
        let stored_runtime = runpod_workspace_runtimes::Entity::find_by_id(&runtime.workspace_id)
            .one(&transaction)
            .await
            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
        let stored_operation = lifecycle_operations::Entity::find_by_id(operation.id.to_string())
            .one(&transaction)
            .await
            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;

        if operation.state == LifecycleOperationState::Failed {
            let mut model = stored_runtime
                .ok_or(RuntimeTransitionRepositoryError::NotFound)?
                .into_active_model();
            model.state = Set("failed".to_owned());
            model
                .update(&transaction)
                .await
                .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
            update_operation(
                stored_operation.ok_or(RuntimeTransitionRepositoryError::NotFound)?,
                operation,
                &transaction,
            )
            .await?;
        } else if runtime.state == RunpodRuntimeState::CleaningUp
            && operation.state == LifecycleOperationState::Succeeded
        {
            if runpod_workspace_runtimes::Entity::delete_by_id(&runtime.workspace_id)
                .exec(&transaction)
                .await
                .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?
                .rows_affected
                == 0
            {
                return Err(RuntimeTransitionRepositoryError::NotFound);
            }
            workspace_runtimes::Entity::delete_by_id(&runtime.workspace_id)
                .exec(&transaction)
                .await
                .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
            update_operation(
                stored_operation.ok_or(RuntimeTransitionRepositoryError::NotFound)?,
                operation,
                &transaction,
            )
            .await?;
        } else {
            match stored_runtime {
                Some(model) => update_runtime(model, runtime, &transaction).await?,
                None if anchor.is_none() => insert_runtime(runtime, &transaction).await?,
                None => return Err(RuntimeTransitionRepositoryError::CorruptData),
            }

            match stored_operation {
                Some(model) => {
                    update_operation(model, operation, &transaction).await?;
                    if operation.state == LifecycleOperationState::Running {
                        let mut progress =
                            runpod_lifecycle_progress::Entity::find_by_id(operation.id.to_string())
                                .one(&transaction)
                                .await
                                .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?
                                .ok_or(RuntimeTransitionRepositoryError::CorruptData)?
                                .into_active_model();
                        progress.step = Set(progress_value(operation.progress).to_owned());
                        progress
                            .update(&transaction)
                            .await
                            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
                    }
                }
                None => insert_operation(operation, &transaction).await?,
            }
        }

        transaction
            .commit()
            .await
            .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)
    }
}

async fn insert_runtime(
    runtime: &RunpodRuntime,
    connection: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimeTransitionRepositoryError> {
    workspace_runtimes::ActiveModel {
        workspace_id: Set(runtime.workspace_id.clone()),
        provider_kind: Set("runpod".to_owned()),
    }
    .insert(connection)
    .await
    .map_err(map_runtime_insert_error)?;
    runpod_workspace_runtimes::ActiveModel {
        workspace_id: Set(runtime.workspace_id.clone()),
        state: Set(runtime_state_value(runtime.state).to_owned()),
        datacenter_id: Set(runtime.config.datacenter_id.clone()),
        gpu_id: Set(runtime.config.gpu_id.clone()),
        volume_size_gb: Set(i64::try_from(runtime.config.volume_size_gb)
            .map_err(|_| RuntimeTransitionRepositoryError::CorruptData)?),
        network_volume_id: Set(runtime.resources.network_volume_id.clone()),
        provisioner_pod_id: Set(runtime.resources.provisioner_pod_id.clone()),
        endpoint_id: Set(runtime.resources.endpoint_id.clone()),
        template_id: Set(runtime.resources.template_id.clone()),
    }
    .insert(connection)
    .await
    .map_err(map_runtime_insert_error)?;
    Ok(())
}

async fn update_runtime(
    model: runpod_workspace_runtimes::Model,
    runtime: &RunpodRuntime,
    connection: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimeTransitionRepositoryError> {
    let mut model = model.into_active_model();
    model.state = Set(runtime_state_value(runtime.state).to_owned());
    model.network_volume_id = Set(runtime.resources.network_volume_id.clone());
    model.provisioner_pod_id = Set(runtime.resources.provisioner_pod_id.clone());
    model.endpoint_id = Set(runtime.resources.endpoint_id.clone());
    model.template_id = Set(runtime.resources.template_id.clone());
    model
        .update(connection)
        .await
        .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
    Ok(())
}

async fn insert_operation(
    operation: &LifecycleOperation,
    connection: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimeTransitionRepositoryError> {
    lifecycle_operations::ActiveModel {
        id: Set(operation.id.to_string()),
        workspace_id: Set(operation.workspace_id.clone()),
        running_workspace_id: Set((operation.state == LifecycleOperationState::Running)
            .then(|| operation.workspace_id.clone())),
        operation_kind: Set(operation_kind_value(operation.kind()).to_owned()),
        state: Set(operation_state_value(operation.state).to_owned()),
        trace_id: Set(operation.trace_id.to_string()),
        created_at: Set(operation.created_at),
        updated_at: Set(operation.updated_at),
        finished_at: Set(operation.finished_at),
    }
    .insert(connection)
    .await
    .map_err(|error| match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => {
            RuntimeTransitionRepositoryError::OperationAlreadyRunning
        }
        _ => RuntimeTransitionRepositoryError::Unavailable,
    })?;
    runpod_lifecycle_progress::ActiveModel {
        operation_id: Set(operation.id.to_string()),
        step: Set(progress_value(operation.progress).to_owned()),
    }
    .insert(connection)
    .await
    .map_err(|_| RuntimeTransitionRepositoryError::Unavailable)?;
    Ok(())
}

async fn update_operation(
    model: lifecycle_operations::Model,
    operation: &LifecycleOperation,
    connection: &sea_orm::DatabaseTransaction,
) -> Result<(), RuntimeTransitionRepositoryError> {
    let mut model = model.into_active_model();
    model.state = Set(operation_state_value(operation.state).to_owned());
    model.running_workspace_id = Set((operation.state == LifecycleOperationState::Running)
        .then(|| operation.workspace_id.clone()));
    model.updated_at = Set(operation.updated_at);
    model.finished_at = Set(operation.finished_at);
    model
        .update(connection)
        .await
        .map_err(|error| match error.sql_err() {
            Some(SqlErr::UniqueConstraintViolation(_)) => {
                RuntimeTransitionRepositoryError::OperationAlreadyRunning
            }
            _ => RuntimeTransitionRepositoryError::Unavailable,
        })?;
    Ok(())
}

fn map_runtime(
    model: runpod_workspace_runtimes::Model,
) -> Result<RunpodRuntime, RunpodRuntimeRepositoryError> {
    let state = match model.state.as_str() {
        "provisioning" => RunpodRuntimeState::Provisioning,
        "ready" => RunpodRuntimeState::Ready,
        "cleaning_up" => RunpodRuntimeState::CleaningUp,
        "failed" => RunpodRuntimeState::Failed,
        _ => return Err(RunpodRuntimeRepositoryError::CorruptData),
    };
    Ok(RunpodRuntime {
        workspace_id: model.workspace_id,
        state,
        config: RunpodRuntimeConfig {
            datacenter_id: model.datacenter_id,
            gpu_id: model.gpu_id,
            volume_size_gb: u64::try_from(model.volume_size_gb)
                .map_err(|_| RunpodRuntimeRepositoryError::CorruptData)?,
        },
        resources: RunpodRuntimeResources {
            network_volume_id: model.network_volume_id,
            provisioner_pod_id: model.provisioner_pod_id,
            template_id: model.template_id,
            endpoint_id: model.endpoint_id,
        },
    })
}

fn runtime_state_value(state: RunpodRuntimeState) -> &'static str {
    match state {
        RunpodRuntimeState::Provisioning => "provisioning",
        RunpodRuntimeState::Ready => "ready",
        RunpodRuntimeState::CleaningUp => "cleaning_up",
        RunpodRuntimeState::Failed => "failed",
    }
}

fn map_runtime_insert_error(error: sea_orm::DbErr) -> RuntimeTransitionRepositoryError {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => {
            RuntimeTransitionRepositoryError::AlreadyExists
        }
        _ => RuntimeTransitionRepositoryError::Unavailable,
    }
}
