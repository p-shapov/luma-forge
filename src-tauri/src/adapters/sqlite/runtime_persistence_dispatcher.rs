use std::collections::HashMap;

use sea_orm::{ConnectionTrait, DatabaseTransaction};

use crate::application::runtimes::{
    ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
    RuntimeKind, RuntimeOperation, RuntimeProgress, RuntimeProvider,
};

use super::runpod_runtime_persistence;

pub(super) fn runtime_kind_value(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Runpod => "runpod",
    }
}

pub(super) fn parse_runtime_kind(value: &str) -> Result<RuntimeKind, RuntimePersistenceError> {
    match value {
        "runpod" => Ok(RuntimeKind::Runpod),
        _ => Err(RuntimePersistenceError::CorruptData),
    }
}

pub(super) async fn save_runtime(
    workspace_id: &str,
    provider: &RuntimeProvider,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    match provider {
        RuntimeProvider::Runpod(runtime) => {
            runpod_runtime_persistence::save_runtime(workspace_id, runtime, transaction).await
        }
    }
}

pub(super) async fn save_progress(
    operation: &RuntimeOperation,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    validate_progress(operation)?;
    match operation.progress {
        RuntimeProgress::Runpod(progress) => {
            runpod_runtime_persistence::save_progress(operation, progress, transaction).await
        }
    }
}

pub(super) fn validate_progress(
    operation: &RuntimeOperation,
) -> Result<(), RuntimePersistenceError> {
    let RuntimeProgress::Runpod(progress) = operation.progress;
    if operation.runtime_kind != RuntimeKind::Runpod
        || match operation.kind {
            crate::application::runtimes::RuntimeOperationKind::Provision => {
                progress.provision_step().is_none()
            }
            crate::application::runtimes::RuntimeOperationKind::Cleanup => {
                progress.cleanup_step().is_none()
            }
        }
    {
        return Err(RuntimePersistenceError::CorruptData);
    }
    Ok(())
}

pub(super) async fn load_runtime<C: ConnectionTrait>(
    workspace_id: &str,
    kind: RuntimeKind,
    connection: &C,
) -> Result<RuntimeProvider, RuntimePersistenceError> {
    match kind {
        RuntimeKind::Runpod => Ok(RuntimeProvider::Runpod(
            runpod_runtime_persistence::load_runtime(workspace_id, connection).await?,
        )),
    }
}

pub(super) async fn load_runtimes<C: ConnectionTrait>(
    runtimes: &[(String, RuntimeKind)],
    connection: &C,
) -> Result<HashMap<String, RuntimeProvider>, RuntimePersistenceError> {
    let runpod_ids = runtimes
        .iter()
        .map(|(id, kind)| match kind {
            RuntimeKind::Runpod => id.clone(),
        })
        .collect::<Vec<_>>();
    Ok(
        runpod_runtime_persistence::load_runtimes(&runpod_ids, connection)
            .await?
            .into_iter()
            .map(|(id, runtime)| (id, RuntimeProvider::Runpod(runtime)))
            .collect(),
    )
}

pub(super) async fn load_progress<C: ConnectionTrait>(
    operations: &[(String, RuntimeKind)],
    connection: &C,
) -> Result<HashMap<String, RuntimeProgress>, RuntimeOperationRepositoryError> {
    let runpod_ids = operations
        .iter()
        .map(|(id, kind)| match kind {
            RuntimeKind::Runpod => id.clone(),
        })
        .collect::<Vec<_>>();
    Ok(
        runpod_runtime_persistence::load_progress(&runpod_ids, connection)
            .await?
            .into_iter()
            .map(|(id, progress)| (id, RuntimeProgress::Runpod(progress)))
            .collect(),
    )
}
