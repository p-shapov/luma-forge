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
        runpod::{RunpodPlacementPlan, RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceCatalog, WorkspaceId},
        workspace::{WorkspaceRuntime, WorkspaceState},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    runtime_catalog::BundledRuntimeCatalogRepository,
    shared::{AppFuture, BackgroundTask, BackgroundTaskSpawner, NoopEventSink},
    workflow_catalog::BundledWorkflowCatalogRepository,
    workspace::{
        events::WorkspaceEvent,
        runtime::{CreateRunpodWorkspaceRequest, WorkspaceRuntime as WorkspaceRuntimeTrait},
        service::{WorkspaceService, WorkspaceServiceDependencies},
        WorkspaceRuntimeContext,
    },
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestBackgroundTaskSpawner;

impl BackgroundTaskSpawner for TestBackgroundTaskSpawner {
    fn spawn(&self, task: BackgroundTask) {
        tokio::spawn(task);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FakeWorkspaceRuntime;

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
        workspace: Workspace,
    ) -> AppFuture<'a, Result<(), crate::workspace::WorkspaceError>> {
        Box::pin(async move { context.delete_workspace(&workspace.id).await })
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

pub fn draft_create_request(workspace_id: &str) -> CreateRunpodWorkspaceRequest {
    CreateRunpodWorkspaceRequest {
        workspace_id: workspace_id.to_string(),
        workflow_preset_id: "comfyui-hidream-o1-dev".to_string(),
        placement: RunpodPlacementPlan {
            data_center_id: "dc".to_string(),
            gpu_type_id: "gpu".to_string(),
            volume_size_gb: 100,
        },
    }
}

pub fn workspace_with_runpod(workspace_id: &str, state: WorkspaceState) -> Workspace {
    Workspace {
        id: workspace_id.to_string(),
        workflow: WorkflowReference {
            id: "comfyui-hidream-o1-dev".to_string(),
            version: "1.0.0".to_string(),
        },
        state,
        runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
            placement: RunpodPlacementPlan {
                data_center_id: "dc".to_string(),
                gpu_type_id: "gpu".to_string(),
                volume_size_gb: 100,
            },
            resources: RunpodResources {
                network_volume_id: None,
                provisioner_pod_id: None,
                endpoint_id: None,
                template_id: None,
            },
        }),
    }
}

pub fn service_with_fake_runtime() -> WorkspaceService<
    InMemoryWorkspaceRepository,
    InMemoryLifecycleJournal,
    BundledWorkflowCatalogRepository,
    BundledRuntimeCatalogRepository,
> {
    let repositories = repositories();
    WorkspaceService::new(WorkspaceServiceDependencies {
        workspace_catalog: repositories.workspace_catalog,
        lifecycle_journal: repositories.lifecycle_journal,
        workflow_catalog: BundledWorkflowCatalogRepository::new(),
        runtime_catalog: BundledRuntimeCatalogRepository::new(),
        runtime_registry: crate::workspace::registry::WorkspaceRuntimeRegistry::new(Arc::new(
            FakeWorkspaceRuntime,
        )),
        lifecycle_operation_registry:
            crate::workspace::service::LifecycleOperationRegistry::default(),
        event_sink: Arc::new(NoopEventSink::<WorkspaceEvent>::new()),
        task_spawner: Arc::new(TestBackgroundTaskSpawner),
    })
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
    delete_error: Arc<Mutex<Option<LifecycleJournalError>>>,
}

impl InMemoryLifecycleJournal {
    pub fn fail_delete_for_workspace_once(&self, error: LifecycleJournalError) {
        *self
            .delete_error
            .lock()
            .expect("delete error lock should succeed") = Some(error);
    }
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
            if operations.values().any(|operation| {
                operation.workspace_id == *workspace_id
                    && operation.state == LifecycleOperationState::Running
            }) {
                return Err(LifecycleJournalError::RunningOperationExists);
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
            if let Some(error) = self
                .delete_error
                .lock()
                .expect("delete error lock should succeed")
                .take()
            {
                return Err(error);
            }
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
