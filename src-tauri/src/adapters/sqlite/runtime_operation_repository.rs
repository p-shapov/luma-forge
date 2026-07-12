use std::collections::HashMap;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

use crate::{
    application::runtimes::{
        ports::{RuntimeOperationRepository, RuntimeOperationRepositoryError},
        runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
        RuntimeOperation, RuntimeOperationKind, RuntimeOperationState, RuntimeProgress,
    },
    infra::sqlite::entities::{runpod_runtime_operation_progress, runtime_operations},
};

pub struct SqliteRuntimeOperationRepository {
    connection: DatabaseConnection,
}

impl SqliteRuntimeOperationRepository {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self { connection }
    }

    async fn load(
        &self,
        query: sea_orm::Select<runtime_operations::Entity>,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        let operations = query
            .all(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?;
        let progress = runpod_runtime_operation_progress::Entity::find()
            .filter(
                runpod_runtime_operation_progress::Column::OperationId
                    .is_in(operations.iter().map(|operation| operation.id.clone())),
            )
            .all(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
            .into_iter()
            .map(|progress| (progress.operation_id, progress.step))
            .collect::<HashMap<_, _>>();

        operations
            .into_iter()
            .map(|operation| {
                let step = progress
                    .get(&operation.id)
                    .ok_or(RuntimeOperationRepositoryError::CorruptData)?;
                map_operation(operation, step)
            })
            .collect()
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl RuntimeOperationRepository for SqliteRuntimeOperationRepository {
    #[diagnostic(show_output, show_error)]
    async fn recent(
        &self,
        #[diagnostic(show)] limit: u64,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.load(
            runtime_operations::Entity::find()
                .order_by_desc(runtime_operations::Column::CreatedAt)
                .limit(limit),
        )
        .await
    }

    #[diagnostic(show_output, show_error)]
    async fn recent_for_workspace(
        &self,
        #[diagnostic(show)] workspace_id: &str,
        #[diagnostic(show)] limit: u64,
    ) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.load(
            runtime_operations::Entity::find()
                .filter(runtime_operations::Column::WorkspaceId.eq(workspace_id))
                .order_by_desc(runtime_operations::Column::CreatedAt)
                .limit(limit),
        )
        .await
    }

    #[diagnostic(show_output, show_error)]
    async fn running(&self) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.load(
            runtime_operations::Entity::find()
                .filter(runtime_operations::Column::RunningWorkspaceId.is_not_null()),
        )
        .await
    }

    #[diagnostic(show_output, show_error)]
    async fn has_running(
        &self,
        #[diagnostic(show)] workspace_id: &str,
    ) -> Result<bool, RuntimeOperationRepositoryError> {
        Ok(runtime_operations::Entity::find()
            .filter(runtime_operations::Column::WorkspaceId.eq(workspace_id))
            .filter(runtime_operations::Column::RunningWorkspaceId.is_not_null())
            .one(&self.connection)
            .await
            .map_err(|_| RuntimeOperationRepositoryError::Unavailable)?
            .is_some())
    }
}

fn map_operation(
    model: runtime_operations::Model,
    step: &str,
) -> Result<RuntimeOperation, RuntimeOperationRepositoryError> {
    let kind = match model.operation_kind.as_str() {
        "provision" => RuntimeOperationKind::Provision,
        "cleanup" => RuntimeOperationKind::Cleanup,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let state = match model.state.as_str() {
        "running" => RuntimeOperationState::Running,
        "succeeded" => RuntimeOperationState::Succeeded,
        "failed" => RuntimeOperationState::Failed,
        _ => return Err(RuntimeOperationRepositoryError::CorruptData),
    };
    let progress = match kind {
        RuntimeOperationKind::Provision => {
            RuntimeProgress::Runpod(RunpodProgress::Provision(match step {
                "create_network_volume" => RunpodProvisionStep::CreateNetworkVolume,
                "start_provisioner_pod" => RunpodProvisionStep::StartProvisionerPod,
                "poll_provisioner" => RunpodProvisionStep::PollProvisioner,
                "terminate_provisioner_pod" => RunpodProvisionStep::TerminateProvisionerPod,
                "create_template" => RunpodProvisionStep::CreateTemplate,
                "create_endpoint" => RunpodProvisionStep::CreateEndpoint,
                _ => return Err(RuntimeOperationRepositoryError::CorruptData),
            }))
        }
        RuntimeOperationKind::Cleanup => {
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(match step {
                "delete_endpoint" => RunpodCleanupStep::DeleteEndpoint,
                "delete_template" => RunpodCleanupStep::DeleteTemplate,
                "terminate_provisioner_pod" => RunpodCleanupStep::TerminateProvisionerPod,
                "delete_network_volume" => RunpodCleanupStep::DeleteNetworkVolume,
                _ => return Err(RuntimeOperationRepositoryError::CorruptData),
            }))
        }
    };

    Ok(RuntimeOperation {
        id: Uuid::parse_str(&model.id).map_err(|_| RuntimeOperationRepositoryError::CorruptData)?,
        workspace_id: model.workspace_id,
        kind,
        state,
        trace_id: model
            .trace_id
            .map(|trace_id| {
                uuid::Uuid::parse_str(&trace_id)
                    .map_err(|_| RuntimeOperationRepositoryError::CorruptData)
            })
            .transpose()?,
        progress,
        created_at: model.created_at,
        updated_at: model.updated_at,
        finished_at: model.finished_at,
    })
}

pub(super) fn runtime_operation_kind_value(kind: RuntimeOperationKind) -> &'static str {
    match kind {
        RuntimeOperationKind::Provision => "provision",
        RuntimeOperationKind::Cleanup => "cleanup",
    }
}

pub(super) fn runtime_operation_state_value(state: RuntimeOperationState) -> &'static str {
    match state {
        RuntimeOperationState::Running => "running",
        RuntimeOperationState::Succeeded => "succeeded",
        RuntimeOperationState::Failed => "failed",
    }
}

pub(super) fn runtime_operation_progress_value(progress: RuntimeProgress) -> &'static str {
    match progress {
        RuntimeProgress::Runpod(RunpodProgress::Provision(step)) => match step {
            RunpodProvisionStep::CreateNetworkVolume => "create_network_volume",
            RunpodProvisionStep::StartProvisionerPod => "start_provisioner_pod",
            RunpodProvisionStep::PollProvisioner => "poll_provisioner",
            RunpodProvisionStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodProvisionStep::CreateTemplate => "create_template",
            RunpodProvisionStep::CreateEndpoint => "create_endpoint",
        },
        RuntimeProgress::Runpod(RunpodProgress::Cleanup(step)) => match step {
            RunpodCleanupStep::DeleteEndpoint => "delete_endpoint",
            RunpodCleanupStep::DeleteTemplate => "delete_template",
            RunpodCleanupStep::TerminateProvisionerPod => "terminate_provisioner_pod",
            RunpodCleanupStep::DeleteNetworkVolume => "delete_network_volume",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(trace_id: Option<&str>) -> runtime_operations::Model {
        runtime_operations::Model {
            id: uuid::Uuid::nil().to_string(),
            workspace_id: "workspace-1".into(),
            running_workspace_id: Some("workspace-1".into()),
            operation_kind: "provision".into(),
            state: "running".into(),
            trace_id: trace_id.map(str::to_owned),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            updated_at: time::OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        }
    }

    #[test]
    fn trace_mapping_accepts_uuid_or_null_and_rejects_invalid_text() {
        let trace_id = uuid::Uuid::new_v4();
        assert_eq!(
            map_operation(model(Some(&trace_id.to_string())), "create_network_volume")
                .unwrap()
                .trace_id,
            Some(trace_id)
        );
        assert_eq!(
            map_operation(model(None), "create_network_volume")
                .unwrap()
                .trace_id,
            None
        );
        assert_eq!(
            map_operation(model(Some("invalid")), "create_network_volume"),
            Err(RuntimeOperationRepositoryError::CorruptData)
        );
    }
}
