use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::LifecycleOperation,
        runpod::RunpodPlacementPlan,
        workspace::{Workspace, WorkspaceId},
    },
    lifecycle_journal::LifecycleJournalRepository,
    shared::{AppFuture, EventSink},
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{errors::WorkspaceError, events::WorkspaceEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset_id: String,
    pub placement: RunpodPlacementPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionWorkspaceResponse {
    pub workspace: Workspace,
    pub operation: LifecycleOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupWorkspaceResponse {
    pub workspace: Workspace,
    pub operation: LifecycleOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteWorkspaceResponse {
    pub workspace_id: WorkspaceId,
}

pub trait WorkspaceRuntime: Send + Sync {
    fn provision<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>>;

    fn cleanup<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>>;

    fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceError>>;
}

#[derive(Clone)]
pub struct WorkspaceRuntimeContext<'a> {
    workspace_catalog: Arc<dyn WorkspaceCatalogRepository + 'a>,
    lifecycle_journal: Arc<dyn LifecycleJournalRepository + 'a>,
    event_sink: Arc<dyn EventSink<WorkspaceEvent> + 'a>,
    trace_id: String,
}

impl<'a> WorkspaceRuntimeContext<'a> {
    pub fn new(
        workspace_catalog: Arc<dyn WorkspaceCatalogRepository + 'a>,
        lifecycle_journal: Arc<dyn LifecycleJournalRepository + 'a>,
        event_sink: Arc<dyn EventSink<WorkspaceEvent> + 'a>,
        trace_id: String,
    ) -> Self {
        Self {
            workspace_catalog,
            lifecycle_journal,
            event_sink,
            trace_id,
        }
    }

    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    pub async fn persist_operation(
        &self,
        operation: LifecycleOperation,
    ) -> Result<LifecycleOperation, WorkspaceError> {
        let operation = match operation.state {
            crate::domain::lifecycle_operation::LifecycleOperationState::Running => {
                self.lifecycle_journal
                    .mark_state(
                        &operation.operation_id,
                        crate::domain::lifecycle_operation::LifecycleOperationState::Running,
                        operation.payload.as_ref(),
                    )
                    .await?
            }
            state => {
                self.lifecycle_journal
                    .mark_state(&operation.operation_id, state, operation.payload.as_ref())
                    .await?
            }
        };
        self.event_sink
            .emit(WorkspaceEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                trace_id: self.trace_id.clone(),
                operation: operation.clone(),
            });
        Ok(operation)
    }

    pub async fn persist_workspace(
        &self,
        workspace: Workspace,
    ) -> Result<Workspace, WorkspaceError> {
        let workspace = self.workspace_catalog.update_workspace(&workspace).await?;
        self.event_sink.emit(WorkspaceEvent::WorkspaceChanged {
            workspace_id: workspace.id.clone(),
            workspace: workspace.clone(),
        });
        Ok(workspace)
    }

    pub async fn delete_workspace(&self, workspace_id: &str) -> Result<(), WorkspaceError> {
        self.workspace_catalog
            .delete_workspace(workspace_id)
            .await?;
        self.event_sink.emit(WorkspaceEvent::WorkspaceDeleted {
            workspace_id: workspace_id.to_string(),
        });
        Ok(())
    }
}

#[cfg(test)]
impl<'a> WorkspaceRuntimeContext<'a> {
    pub async fn insert_workspace_for_test(&self, workspace: Workspace) {
        self.workspace_catalog
            .insert_workspace(&workspace)
            .await
            .expect("workspace insert should succeed");
    }

    pub async fn find_workspace_for_test(&self, workspace_id: &str) -> Option<Workspace> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .expect("workspace lookup should succeed")
    }

    pub async fn create_operation_for_test(&self, workspace_id: &str) -> LifecycleOperation {
        self.lifecycle_journal
            .create_operation(&workspace_id.to_string())
            .await
            .expect("operation create should succeed")
    }

    pub async fn latest_operation_for_test(
        &self,
        workspace_id: &str,
    ) -> Option<LifecycleOperation> {
        self.lifecycle_journal
            .latest_for_workspace(&workspace_id.to_string())
            .await
            .expect("latest operation should load")
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleOperationPayload, LifecycleOperationState, LifecycleProvisionPayload,
            },
            runpod::{RunpodLifecycleProvisionPayload, RunpodProvisionStep},
        },
        workspace::test_support::runtime_context_for_test,
    };

    #[tokio::test]
    async fn persist_running_operation_refreshes_updated_at_for_progress_payload() {
        let context = runtime_context_for_test();
        let mut operation = context.create_operation_for_test("workspace-1").await;
        let original_updated_at = operation.updated_at;
        operation.payload = Some(LifecycleOperationPayload::Provision(
            LifecycleProvisionPayload::Runpod(RunpodLifecycleProvisionPayload {
                step: Some(RunpodProvisionStep::CreateNetworkVolume),
            }),
        ));
        operation.state = LifecycleOperationState::Running;

        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        let updated = context
            .persist_operation(operation)
            .await
            .expect("operation should persist");

        assert!(updated.updated_at > original_updated_at);
    }
}
