use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::{
    application::lifecycle::{
        ports::{LifecycleOperationRepository, LifecycleOperationRepositoryError},
        progress::runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
        LifecycleOperation, LifecycleOperationKind, LifecycleOperationState, LifecycleProgress,
    },
    infra::sqlite::entities::{lifecycle_operations, runpod_lifecycle_progress},
};

pub struct SqliteLifecycleOperationRepository {
    connection: DatabaseConnection,
}

impl SqliteLifecycleOperationRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    async fn load(
        &self,
        query: sea_orm::Select<lifecycle_operations::Entity>,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        let operations = query
            .all(&self.connection)
            .await
            .map_err(|_| LifecycleOperationRepositoryError::Unavailable)?;
        let progress = runpod_lifecycle_progress::Entity::find()
            .filter(
                runpod_lifecycle_progress::Column::OperationId
                    .is_in(operations.iter().map(|operation| operation.id.clone())),
            )
            .all(&self.connection)
            .await
            .map_err(|_| LifecycleOperationRepositoryError::Unavailable)?
            .into_iter()
            .map(|progress| (progress.operation_id, progress.step))
            .collect::<HashMap<_, _>>();

        operations
            .into_iter()
            .map(|operation| {
                let step = progress
                    .get(&operation.id)
                    .ok_or(LifecycleOperationRepositoryError::CorruptData)?;
                map_operation(operation, step)
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LifecycleOperationRepository for SqliteLifecycleOperationRepository {
    async fn recent(
        &self,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        self.load(
            lifecycle_operations::Entity::find()
                .order_by_desc(lifecycle_operations::Column::CreatedAt)
                .limit(limit),
        )
        .await
    }

    async fn recent_for_workspace(
        &self,
        workspace_id: &str,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        self.load(
            lifecycle_operations::Entity::find()
                .filter(lifecycle_operations::Column::WorkspaceId.eq(workspace_id))
                .order_by_desc(lifecycle_operations::Column::CreatedAt)
                .limit(limit),
        )
        .await
    }

    async fn running(&self) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        self.load(
            lifecycle_operations::Entity::find()
                .filter(lifecycle_operations::Column::RunningWorkspaceId.is_not_null()),
        )
        .await
    }

    async fn has_running(
        &self,
        workspace_id: &str,
    ) -> Result<bool, LifecycleOperationRepositoryError> {
        Ok(lifecycle_operations::Entity::find()
            .filter(lifecycle_operations::Column::WorkspaceId.eq(workspace_id))
            .filter(lifecycle_operations::Column::RunningWorkspaceId.is_not_null())
            .one(&self.connection)
            .await
            .map_err(|_| LifecycleOperationRepositoryError::Unavailable)?
            .is_some())
    }
}

fn map_operation(
    model: lifecycle_operations::Model,
    step: &str,
) -> Result<LifecycleOperation, LifecycleOperationRepositoryError> {
    let kind = match model.operation_kind.as_str() {
        "provision" => LifecycleOperationKind::Provision,
        "cleanup" => LifecycleOperationKind::Cleanup,
        _ => return Err(LifecycleOperationRepositoryError::CorruptData),
    };
    let state = match model.state.as_str() {
        "running" => LifecycleOperationState::Running,
        "succeeded" => LifecycleOperationState::Succeeded,
        "failed" => LifecycleOperationState::Failed,
        _ => return Err(LifecycleOperationRepositoryError::CorruptData),
    };
    let progress = match kind {
        LifecycleOperationKind::Provision => {
            LifecycleProgress::Runpod(RunpodProgress::Provision(match step {
                "create_network_volume" => RunpodProvisionStep::CreateNetworkVolume,
                "start_provisioner_pod" => RunpodProvisionStep::StartProvisionerPod,
                "poll_provisioner" => RunpodProvisionStep::PollProvisioner,
                "terminate_provisioner_pod" => RunpodProvisionStep::TerminateProvisionerPod,
                "create_template" => RunpodProvisionStep::CreateTemplate,
                "create_endpoint" => RunpodProvisionStep::CreateEndpoint,
                _ => return Err(LifecycleOperationRepositoryError::CorruptData),
            }))
        }
        LifecycleOperationKind::Cleanup => {
            LifecycleProgress::Runpod(RunpodProgress::Cleanup(match step {
                "delete_endpoint" => RunpodCleanupStep::DeleteEndpoint,
                "delete_template" => RunpodCleanupStep::DeleteTemplate,
                "terminate_provisioner_pod" => RunpodCleanupStep::TerminateProvisionerPod,
                "delete_network_volume" => RunpodCleanupStep::DeleteNetworkVolume,
                _ => return Err(LifecycleOperationRepositoryError::CorruptData),
            }))
        }
    };

    Ok(LifecycleOperation {
        id: Uuid::parse_str(&model.id)
            .map_err(|_| LifecycleOperationRepositoryError::CorruptData)?,
        workspace_id: model.workspace_id,
        state,
        trace_id: Uuid::parse_str(&model.trace_id)
            .map_err(|_| LifecycleOperationRepositoryError::CorruptData)?,
        progress,
        created_at: model.created_at,
        updated_at: model.updated_at,
        finished_at: model.finished_at,
    })
}

pub(super) fn operation_kind_value(kind: LifecycleOperationKind) -> &'static str {
    match kind {
        LifecycleOperationKind::Provision => "provision",
        LifecycleOperationKind::Cleanup => "cleanup",
    }
}

pub(super) fn operation_state_value(state: LifecycleOperationState) -> &'static str {
    match state {
        LifecycleOperationState::Running => "running",
        LifecycleOperationState::Succeeded => "succeeded",
        LifecycleOperationState::Failed => "failed",
    }
}

pub(super) fn progress_value(progress: LifecycleProgress) -> &'static str {
    match progress {
        LifecycleProgress::Runpod(RunpodProgress::Provision(step)) => match step {
            RunpodProvisionStep::CreateNetworkVolume => "create_network_volume",
            RunpodProvisionStep::StartProvisionerPod => "start_provisioner_pod",
            RunpodProvisionStep::PollProvisioner => "poll_provisioner",
            RunpodProvisionStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodProvisionStep::CreateTemplate => "create_template",
            RunpodProvisionStep::CreateEndpoint => "create_endpoint",
        },
        LifecycleProgress::Runpod(RunpodProgress::Cleanup(step)) => match step {
            RunpodCleanupStep::DeleteEndpoint => "delete_endpoint",
            RunpodCleanupStep::DeleteTemplate => "delete_template",
            RunpodCleanupStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodCleanupStep::DeleteNetworkVolume => "delete_network_volume",
        },
    }
}
