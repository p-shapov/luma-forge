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
    shared::{AppFuture, BackgroundTask, BackgroundTaskSpawner, NoopEventSink},
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::{
        events::WorkspaceEvent,
        runtime::WorkspaceRuntime as WorkspaceRuntimeTrait,
        service::{WorkspaceService, WorkspaceServiceDependencies},
        WorkspaceRuntimeContext,
    },
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TestBackgroundTaskSpawner;

impl BackgroundTaskSpawner for TestBackgroundTaskSpawner {
    fn spawn(&self, task: BackgroundTask) {
        tokio::spawn(task);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FakeWorkspaceRuntime;

impl WorkspaceRuntimeTrait for FakeWorkspaceRuntime {
    fn provision<'a>(
        &'a self,
        _context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
        Box::pin(async move { Ok(workspace) })
    }

    fn cleanup<'a>(
        &'a self,
        _context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
        Box::pin(async move { Ok(workspace) })
    }

    fn delete<'a>(
        &'a self,
        context: WorkspaceRuntimeContext<'a>,
        _operation: LifecycleOperation,
        workspace: Workspace,
    ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
        Box::pin(async move {
            context.delete_workspace(&workspace.id).await?;
            Ok(workspace)
        })
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
        runtime,
        event_sink: Arc::new(NoopEventSink::<WorkspaceEvent>::new()),
        task_spawner: Arc::new(TestBackgroundTaskSpawner),
    });
    (service, repositories)
}

pub fn runtime_context_for_test<'a>() -> WorkspaceRuntimeContext<'a> {
    let repositories = repositories();
    WorkspaceRuntimeContext::new(
        repositories.workspace_catalog,
        repositories.lifecycle_journal,
        Arc::new(NoopEventSink::<WorkspaceEvent>::new()),
        "trace-test".to_string(),
    )
}

#[derive(Clone, Default)]
pub struct InMemoryWorkspaceRepository {
    workspaces: Arc<Mutex<HashMap<String, Workspace>>>,
}

impl WorkspaceCatalogRepository for InMemoryWorkspaceRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            Ok(WorkspaceCatalog {
                workspaces: self
                    .workspaces
                    .lock()
                    .expect("workspace lock should succeed")
                    .values()
                    .cloned()
                    .collect(),
            })
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>> {
        Box::pin(async move {
            Ok(self
                .workspaces
                .lock()
                .expect("workspace lock should succeed")
                .get(id)
                .cloned())
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            let mut workspaces = self
                .workspaces
                .lock()
                .expect("workspace lock should succeed");
            if workspaces.contains_key(&workspace.id) {
                return Err(WorkspaceCatalogError::WorkspaceAlreadyExists);
            }

            workspaces.insert(workspace.id.clone(), workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            let mut workspaces = self
                .workspaces
                .lock()
                .expect("workspace lock should succeed");
            if !workspaces.contains_key(&workspace.id) {
                return Err(WorkspaceCatalogError::WorkspaceNotFound);
            }

            workspaces.insert(workspace.id.clone(), workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn delete_workspace<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
        Box::pin(async move {
            self.workspaces
                .lock()
                .expect("workspace lock should succeed")
                .remove(id)
                .map(|_| ())
                .ok_or(WorkspaceCatalogError::WorkspaceNotFound)
        })
    }
}

#[derive(Clone, Default)]
pub struct InMemoryLifecycleJournal {
    operations: Arc<Mutex<HashMap<String, LifecycleOperation>>>,
}

impl LifecycleJournalRepository for InMemoryLifecycleJournal {
    fn create_operation<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
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
        })
    }

    fn find_running_by_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
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
        })
    }

    fn list_running<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<Vec<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
            Ok(self
                .operations
                .lock()
                .expect("operation lock should succeed")
                .values()
                .filter(|operation| operation.state == LifecycleOperationState::Running)
                .cloned()
                .collect())
        })
    }

    fn latest_for_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<Option<LifecycleOperation>, LifecycleJournalError>> {
        Box::pin(async move {
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
        })
    }

    fn delete_for_workspace<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
    ) -> AppFuture<'a, Result<(), LifecycleJournalError>> {
        Box::pin(async move {
            self.operations
                .lock()
                .expect("operation lock should succeed")
                .retain(|_, operation| operation.workspace_id != *workspace_id);
            Ok(())
        })
    }

    fn update_operation<'a>(
        &'a self,
        operation: &'a LifecycleOperation,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
            let mut operations = self
                .operations
                .lock()
                .expect("operation lock should succeed");
            if !operations.contains_key(&operation.operation_id) {
                return Err(LifecycleJournalError::OperationNotFound);
            }

            operations.insert(operation.operation_id.clone(), operation.clone());
            Ok(operation.clone())
        })
    }

    fn mark_state<'a>(
        &'a self,
        operation_id: &'a LifecycleOperationId,
        state: LifecycleOperationState,
        payload: Option<&'a LifecycleOperationPayload>,
    ) -> AppFuture<'a, Result<LifecycleOperation, LifecycleJournalError>> {
        Box::pin(async move {
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
        })
    }
}
