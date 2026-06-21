use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use time::OffsetDateTime;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState,
        },
        workspace::{Workspace, WorkspaceCatalog, WorkspaceId},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::{
        events::NoopWorkspaceEventSink,
        runtime::{
            WorkspaceRuntime as WorkspaceRuntimeTrait, WorkspaceRuntimeDispatcher,
            WorkspaceRuntimeImplementations,
        },
        service::{WorkspaceService, WorkspaceServiceDependencies},
        WorkspaceRuntimeContext,
    },
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FakeWorkspaceRuntime;

#[async_trait::async_trait]
impl WorkspaceRuntimeTrait for FakeWorkspaceRuntime {
    async fn provision<'a>(
        &'a self,
        _context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, crate::workspace::WorkspaceError> {
        Ok(workspace)
    }

    async fn cleanup<'a>(
        &'a self,
        _context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, crate::workspace::WorkspaceError> {
        Ok(workspace)
    }

    async fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, crate::workspace::WorkspaceError> {
        context.delete_workspace(&workspace.id).await?;
        Ok(workspace)
    }
}

pub struct TestRepositories {
    pub workspace_catalog: Arc<InMemoryWorkspaceRepository>,
    pub lifecycle_journal: Arc<InMemoryLifecycleJournal>,
}

pub fn repositories() -> TestRepositories {
    TestRepositories {
        workspace_catalog: Arc::new(InMemoryWorkspaceRepository::default()),
        lifecycle_journal: Arc::new(InMemoryLifecycleJournal::default()),
    }
}

pub(crate) fn service_with_runtime(
    runtime: Arc<dyn WorkspaceRuntimeTrait>,
) -> (WorkspaceService, TestRepositories) {
    let repositories = repositories();
    let service = WorkspaceService::new(WorkspaceServiceDependencies {
        workspace_catalog: repositories.workspace_catalog.clone(),
        lifecycle_journal: repositories.lifecycle_journal.clone(),
        workflow_catalog: Arc::new(BundledWorkflowCatalogRepository::new()),
        runtime_dispatcher: WorkspaceRuntimeDispatcher::new(WorkspaceRuntimeImplementations {
            runpod: runtime,
        }),
        event_sink: Arc::new(NoopWorkspaceEventSink),
    });
    (service, repositories)
}

pub fn runtime_context_for_test<'a>() -> WorkspaceRuntimeContext<'a> {
    let repositories = repositories();
    WorkspaceRuntimeContext::new(
        repositories.workspace_catalog,
        repositories.lifecycle_journal,
        Arc::new(NoopWorkspaceEventSink),
    )
}

#[derive(Clone, Default)]
pub struct InMemoryWorkspaceRepository {
    workspaces: Arc<Mutex<HashMap<String, Workspace>>>,
}

#[async_trait::async_trait]
impl WorkspaceCatalogRepository for InMemoryWorkspaceRepository {
    async fn list_workspaces(&self) -> Result<WorkspaceCatalog, WorkspaceCatalogError> {
        Ok(WorkspaceCatalog {
            workspaces: self
                .workspaces
                .lock()
                .expect("workspace lock should succeed")
                .values()
                .cloned()
                .collect(),
        })
    }

    async fn find_workspace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<Workspace>, WorkspaceCatalogError> {
        Ok(self
            .workspaces
            .lock()
            .expect("workspace lock should succeed")
            .get(id)
            .cloned())
    }

    async fn insert_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        let mut workspaces = self
            .workspaces
            .lock()
            .expect("workspace lock should succeed");
        if workspaces.contains_key(&workspace.id) {
            return Err(WorkspaceCatalogError::WorkspaceAlreadyExists);
        }

        workspaces.insert(workspace.id.clone(), workspace.clone());
        Ok(workspace.clone())
    }

    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        let mut workspaces = self
            .workspaces
            .lock()
            .expect("workspace lock should succeed");
        if !workspaces.contains_key(&workspace.id) {
            return Err(WorkspaceCatalogError::WorkspaceNotFound);
        }

        workspaces.insert(workspace.id.clone(), workspace.clone());
        Ok(workspace.clone())
    }

    async fn delete_workspace(&self, id: &str) -> Result<(), WorkspaceCatalogError> {
        self.workspaces
            .lock()
            .expect("workspace lock should succeed")
            .remove(id)
            .map(|_| ())
            .ok_or(WorkspaceCatalogError::WorkspaceNotFound)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryLifecycleJournal {
    operations: Arc<Mutex<HashMap<String, LifecycleOperation>>>,
}

#[async_trait::async_trait]
impl LifecycleJournalRepository for InMemoryLifecycleJournal {
    async fn create_operation(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<LifecycleOperation, LifecycleJournalError> {
        let now = OffsetDateTime::now_utc();
        let mut operations = self
            .operations
            .lock()
            .expect("operation lock should succeed");
        if let Some(operation) = operations.values().find(|operation| {
            operation.workspace_id == *workspace_id
                && operation.state == LifecycleOperationState::Running
        }) {
            return Err(LifecycleJournalError::RunningOperationExists {
                operation_id: operation.operation_id.clone(),
            });
        }

        let operation = LifecycleOperation {
            operation_id: format!("operation-{}", operations.len() + 1),
            workspace_id: workspace_id.clone(),
            state: LifecycleOperationState::Running,
            payload: None,
            created_at: now,
            updated_at: now,
            finished_at: None,
        };
        operations.insert(operation.operation_id.clone(), operation.clone());
        Ok(operation)
    }

    async fn find_running_by_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<LifecycleOperation>, LifecycleJournalError> {
        Ok(self
            .operations
            .lock()
            .expect("operation lock should succeed")
            .values()
            .find(|operation| {
                operation.workspace_id == *workspace_id
                    && operation.state == LifecycleOperationState::Running
            })
            .cloned())
    }

    async fn list_running(&self) -> Result<Vec<LifecycleOperation>, LifecycleJournalError> {
        Ok(self
            .operations
            .lock()
            .expect("operation lock should succeed")
            .values()
            .filter(|operation| operation.state == LifecycleOperationState::Running)
            .cloned()
            .collect())
    }

    async fn latest_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<LifecycleOperation>, LifecycleJournalError> {
        Ok(self
            .operations
            .lock()
            .expect("operation lock should succeed")
            .values()
            .filter(|operation| operation.workspace_id == *workspace_id)
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.updated_at.cmp(&right.updated_at))
                    .then_with(|| left.operation_id.cmp(&right.operation_id))
            })
            .cloned())
    }

    async fn delete_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(), LifecycleJournalError> {
        self.operations
            .lock()
            .expect("operation lock should succeed")
            .retain(|_, operation| operation.workspace_id != *workspace_id);
        Ok(())
    }

    async fn update_operation(
        &self,
        operation: &LifecycleOperation,
    ) -> Result<LifecycleOperation, LifecycleJournalError> {
        let mut operations = self
            .operations
            .lock()
            .expect("operation lock should succeed");
        if !operations.contains_key(&operation.operation_id) {
            return Err(LifecycleJournalError::OperationNotFound);
        }

        operations.insert(operation.operation_id.clone(), operation.clone());
        Ok(operation.clone())
    }

    async fn mark_state(
        &self,
        operation_id: &LifecycleOperationId,
        state: LifecycleOperationState,
        payload: Option<&LifecycleOperationPayload>,
    ) -> Result<LifecycleOperation, LifecycleJournalError> {
        let mut operations = self
            .operations
            .lock()
            .expect("operation lock should succeed");
        let operation = operations
            .get_mut(operation_id)
            .ok_or(LifecycleJournalError::OperationNotFound)?;
        if operation.state != LifecycleOperationState::Running {
            return Err(LifecycleJournalError::OperationNotFound);
        }

        operation.state = state;
        operation.payload = payload.cloned();
        operation.updated_at = OffsetDateTime::now_utc();
        if state != LifecycleOperationState::Running {
            operation.finished_at = Some(operation.updated_at);
        }
        Ok(operation.clone())
    }
}
