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
        workspace: Workspace,
    ) -> AppFuture<'a, Result<(), WorkspaceError>>;
}

#[derive(Clone)]
pub struct WorkspaceRuntimeContext<'a> {
    workspace_catalog: Arc<dyn WorkspaceCatalogRepository + 'a>,
    lifecycle_journal: Arc<dyn LifecycleJournalRepository + 'a>,
    event_sink: Arc<dyn EventSink<WorkspaceEvent> + 'a>,
}

impl<'a> WorkspaceRuntimeContext<'a> {
    pub fn new(
        workspace_catalog: Arc<dyn WorkspaceCatalogRepository + 'a>,
        lifecycle_journal: Arc<dyn LifecycleJournalRepository + 'a>,
        event_sink: Arc<dyn EventSink<WorkspaceEvent> + 'a>,
    ) -> Self {
        Self {
            workspace_catalog,
            lifecycle_journal,
            event_sink,
        }
    }

    pub async fn persist_operation(
        &self,
        operation: LifecycleOperation,
    ) -> Result<LifecycleOperation, WorkspaceError> {
        let operation = match operation.state {
            crate::domain::lifecycle_operation::LifecycleOperationState::Running => self
                .lifecycle_journal
                .update_operation(&operation)
                .await
                .map_err(super::errors::lifecycle_journal_error)?,
            state => self
                .lifecycle_journal
                .mark_state(&operation.operation_id, state, operation.payload.as_ref())
                .await
                .map_err(super::errors::lifecycle_journal_error)?,
        };
        self.event_sink
            .emit(WorkspaceEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                diagnostic_id: None,
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
            workspace: Box::new(workspace.clone()),
        });
        Ok(workspace)
    }

    pub async fn delete_workspace(&self, workspace_id: &str) -> Result<(), WorkspaceError> {
        self.lifecycle_journal
            .delete_for_workspace(&workspace_id.to_string())
            .await
            .map_err(super::errors::lifecycle_journal_error)?;
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
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleCleanupPayload, LifecycleOperationPayload, LifecycleOperationState,
            },
            runpod::{
                RunpodCleanupStep, RunpodLifecycleCleanupPayload, RunpodPlacementPlan,
                RunpodResources, RunpodRuntime,
            },
            workflow_preset::WorkflowReference,
            workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
        },
        lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
        shared::EventSink,
        workspace::{events::WorkspaceEvent, WorkspaceError},
        workspace_catalog::WorkspaceCatalogRepository,
    };

    use super::WorkspaceRuntimeContext;

    #[derive(Default)]
    struct Events(Mutex<Vec<WorkspaceEvent>>);

    impl EventSink<WorkspaceEvent> for Events {
        fn emit(&self, event: WorkspaceEvent) {
            self.0.lock().expect("events lock").push(event);
        }
    }

    #[tokio::test]
    async fn persist_operation_emits_changed_event() {
        let repositories = crate::workspace::test_support::repositories();
        let events = Arc::new(Events::default());
        let context = WorkspaceRuntimeContext::new(
            repositories.workspace_catalog.clone(),
            repositories.lifecycle_journal.clone(),
            events.clone(),
        );
        let mut operation = repositories
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation");
        operation.payload = Some(LifecycleOperationPayload::Cleanup(
            LifecycleCleanupPayload::Runpod(RunpodLifecycleCleanupPayload {
                step: Some(RunpodCleanupStep::DeleteEndpoint),
            }),
        ));

        let persisted = context
            .persist_operation(operation)
            .await
            .expect("persist operation");

        assert_eq!(persisted.state, LifecycleOperationState::Running);
        assert_eq!(events.0.lock().expect("events lock").len(), 1);
    }

    #[tokio::test]
    async fn persist_operation_marks_terminal_states_with_finished_at() {
        let repositories = crate::workspace::test_support::repositories();
        let events = Arc::new(Events::default());
        let context = WorkspaceRuntimeContext::new(
            repositories.workspace_catalog.clone(),
            repositories.lifecycle_journal.clone(),
            events,
        );
        let mut operation = repositories
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string())
            .await
            .expect("operation");
        operation.state = LifecycleOperationState::Completed;
        operation.payload = Some(LifecycleOperationPayload::Cleanup(
            LifecycleCleanupPayload::Runpod(RunpodLifecycleCleanupPayload {
                step: Some(RunpodCleanupStep::DeleteEndpoint),
            }),
        ));
        operation.finished_at = None;

        let persisted = context
            .persist_operation(operation)
            .await
            .expect("persist operation");

        assert_eq!(persisted.state, LifecycleOperationState::Completed);
        assert!(persisted.finished_at.is_some());
        assert_eq!(persisted.finished_at, Some(persisted.updated_at));
    }

    #[tokio::test]
    async fn delete_workspace_keeps_workspace_when_lifecycle_delete_fails() {
        let repositories = crate::workspace::test_support::repositories();
        let events = Arc::new(Events::default());
        let context = WorkspaceRuntimeContext::new(
            repositories.workspace_catalog.clone(),
            repositories.lifecycle_journal.clone(),
            events.clone(),
        );
        let workspace = Workspace {
            id: "workspace-1".to_string(),
            workflow: WorkflowReference {
                id: "workflow-1".to_string(),
                version: "1".to_string(),
            },
            state: WorkspaceState::Ready,
            runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
                placement: RunpodPlacementPlan {
                    data_center_id: "dc-1".to_string(),
                    gpu_type_id: "gpu-1".to_string(),
                    volume_size_gb: 100,
                },
                resources: RunpodResources {
                    network_volume_id: None,
                    provisioner_pod_id: None,
                    endpoint_id: None,
                    template_id: None,
                },
            }),
        };
        repositories
            .workspace_catalog
            .insert_workspace(&workspace)
            .await
            .expect("workspace");
        repositories
            .lifecycle_journal
            .create_operation(&workspace.id)
            .await
            .expect("operation");
        repositories
            .lifecycle_journal
            .fail_delete_for_workspace_once(LifecycleJournalError::StorageUnavailable {
                message: "boom".to_string(),
            });

        let error = context
            .delete_workspace(&workspace.id)
            .await
            .expect_err("delete should fail");

        assert!(matches!(
            error,
            WorkspaceError::LifecycleJournalInvalid { .. }
        ));
        assert!(repositories
            .workspace_catalog
            .find_workspace_by_id(&workspace.id)
            .await
            .expect("find workspace")
            .is_some());
        assert_eq!(events.0.lock().expect("events lock").len(), 0);
    }
}
