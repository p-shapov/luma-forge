use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseTransaction,
    EntityTrait, IntoActiveModel, QueryFilter,
};

use crate::{
    application::runtimes::{
        ports::{RuntimeOperationRepositoryError, RuntimePersistenceError},
        runpod::{
            RunpodCleanupStep, RunpodProgress, RunpodProvisionStep, RunpodRuntime,
            RunpodRuntimeConfig, RunpodRuntimeResources,
        },
        RuntimeOperation,
    },
    infra::sqlite::entities::{
        runpod_runtime_operation_progress, runpod_workspace_runtimes, runtime_operations,
    },
};

pub(super) async fn load_runtime<C: ConnectionTrait>(
    workspace_id: &str,
    connection: &C,
) -> Result<RunpodRuntime, RuntimePersistenceError> {
    let model = runpod_workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(connection)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
        .ok_or(RuntimePersistenceError::CorruptData)?;
    map_runtime(model)
}

pub(super) async fn load_runtimes<C: ConnectionTrait>(
    workspace_ids: &[String],
    connection: &C,
) -> Result<HashMap<String, RunpodRuntime>, RuntimePersistenceError> {
    runpod_workspace_runtimes::Entity::find()
        .filter(runpod_workspace_runtimes::Column::WorkspaceId.is_in(workspace_ids.iter().cloned()))
        .all(connection)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
        .into_iter()
        .map(|model| Ok((model.workspace_id.clone(), map_runtime(model)?)))
        .collect()
}

pub(super) async fn save_runtime(
    workspace_id: &str,
    runtime: &RunpodRuntime,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    let volume_size_gb = i64::try_from(runtime.config.volume_size_gb)
        .map_err(|_| RuntimePersistenceError::CorruptData)?;
    match runpod_workspace_runtimes::Entity::find_by_id(workspace_id)
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            let mut model = model.into_active_model();
            model.datacenter_id = Set(runtime.config.datacenter_id.clone());
            model.gpu_id = Set(runtime.config.gpu_id.clone());
            model.volume_size_gb = Set(volume_size_gb);
            model.network_volume_id = Set(runtime.resources.network_volume_id.clone());
            model.provisioner_pod_id = Set(runtime.resources.provisioner_pod_id.clone());
            model.template_id = Set(runtime.resources.template_id.clone());
            model.endpoint_id = Set(runtime.resources.endpoint_id.clone());
            model
                .update(transaction)
                .await
                .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
        None => {
            runpod_workspace_runtimes::ActiveModel {
                workspace_id: Set(workspace_id.to_owned()),
                datacenter_id: Set(runtime.config.datacenter_id.clone()),
                gpu_id: Set(runtime.config.gpu_id.clone()),
                volume_size_gb: Set(volume_size_gb),
                network_volume_id: Set(runtime.resources.network_volume_id.clone()),
                provisioner_pod_id: Set(runtime.resources.provisioner_pod_id.clone()),
                template_id: Set(runtime.resources.template_id.clone()),
                endpoint_id: Set(runtime.resources.endpoint_id.clone()),
            }
            .insert(transaction)
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
    }
    Ok(())
}

pub(super) async fn save_progress(
    operation: &RuntimeOperation,
    progress: RunpodProgress,
    transaction: &DatabaseTransaction,
) -> Result<(), RuntimePersistenceError> {
    let step = progress_value(progress).to_owned();
    match runpod_runtime_operation_progress::Entity::find_by_id(operation.id.to_string())
        .one(transaction)
        .await
        .map_err(|_| RuntimePersistenceError::Unavailable)?
    {
        Some(model) => {
            let mut model = model.into_active_model();
            model.step = Set(step);
            model
                .update(transaction)
                .await
                .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
        None => {
            runpod_runtime_operation_progress::ActiveModel {
                operation_id: Set(operation.id.to_string()),
                step: Set(step),
            }
            .insert(transaction)
            .await
            .map_err(|_| RuntimePersistenceError::Unavailable)?;
        }
    }
    Ok(())
}

pub(super) async fn load_progress<C: ConnectionTrait>(
    operation_ids: &[String],
    connection: &C,
) -> Result<HashMap<String, RunpodProgress>, RuntimeOperationRepositoryError> {
    let kinds = runtime_operations::Entity::find()
        .filter(runtime_operations::Column::Id.is_in(operation_ids.iter().cloned()))
        .all(connection)
        .await
        .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
        .into_iter()
        .map(|model| (model.id, model.operation_kind))
        .collect::<HashMap<_, _>>();
    runpod_runtime_operation_progress::Entity::find()
        .filter(
            runpod_runtime_operation_progress::Column::OperationId
                .is_in(operation_ids.iter().cloned()),
        )
        .all(connection)
        .await
        .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
        .into_iter()
        .map(|model| {
            let kind = kinds
                .get(&model.operation_id)
                .ok_or(RuntimeOperationRepositoryError::CorruptData)?;
            Ok((model.operation_id, parse_progress(kind, &model.step)?))
        })
        .collect()
}

fn map_runtime(
    model: runpod_workspace_runtimes::Model,
) -> Result<RunpodRuntime, RuntimePersistenceError> {
    Ok(RunpodRuntime {
        config: RunpodRuntimeConfig {
            datacenter_id: model.datacenter_id,
            gpu_id: model.gpu_id,
            volume_size_gb: u64::try_from(model.volume_size_gb)
                .map_err(|_| RuntimePersistenceError::CorruptData)?,
        },
        resources: RunpodRuntimeResources {
            network_volume_id: model.network_volume_id,
            provisioner_pod_id: model.provisioner_pod_id,
            template_id: model.template_id,
            endpoint_id: model.endpoint_id,
        },
    })
}

fn progress_value(progress: RunpodProgress) -> &'static str {
    match progress {
        RunpodProgress::Provision(step) => match step {
            RunpodProvisionStep::CreateNetworkVolume => "create_network_volume",
            RunpodProvisionStep::StartProvisionerPod => "start_provisioner_pod",
            RunpodProvisionStep::PollProvisioner => "poll_provisioner",
            RunpodProvisionStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodProvisionStep::CreateTemplate => "create_template",
            RunpodProvisionStep::CreateEndpoint => "create_endpoint",
        },
        RunpodProgress::Cleanup(step) => match step {
            RunpodCleanupStep::DeleteEndpoint => "delete_endpoint",
            RunpodCleanupStep::DeleteTemplate => "delete_template",
            RunpodCleanupStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodCleanupStep::DeleteNetworkVolume => "delete_network_volume",
        },
    }
}

fn parse_progress(
    operation_kind: &str,
    value: &str,
) -> Result<RunpodProgress, RuntimeOperationRepositoryError> {
    Ok(match (operation_kind, value) {
        ("provision", "create_network_volume") => {
            RunpodProgress::Provision(RunpodProvisionStep::CreateNetworkVolume)
        }
        ("provision", "start_provisioner_pod") => {
            RunpodProgress::Provision(RunpodProvisionStep::StartProvisionerPod)
        }
        ("provision", "poll_provisioner") => {
            RunpodProgress::Provision(RunpodProvisionStep::PollProvisioner)
        }
        ("provision", "terminate_provisioner_pod") => {
            RunpodProgress::Provision(RunpodProvisionStep::TerminateProvisionerPod)
        }
        ("provision", "create_template") => {
            RunpodProgress::Provision(RunpodProvisionStep::CreateTemplate)
        }
        ("provision", "create_endpoint") => {
            RunpodProgress::Provision(RunpodProvisionStep::CreateEndpoint)
        }
        ("cleanup", "delete_endpoint") => {
            RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint)
        }
        ("cleanup", "delete_template") => {
            RunpodProgress::Cleanup(RunpodCleanupStep::DeleteTemplate)
        }
        ("cleanup", "terminate_provisioner_pod") => {
            RunpodProgress::Cleanup(RunpodCleanupStep::TerminateProvisionerPod)
        }
        ("cleanup", "delete_network_volume") => {
            RunpodProgress::Cleanup(RunpodCleanupStep::DeleteNetworkVolume)
        }
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    })
}
