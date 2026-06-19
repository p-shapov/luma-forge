use std::sync::Arc;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationState},
        runpod::{RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceRuntime as WorkspaceRuntimeDomain, WorkspaceState},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    runtime_catalog::RuntimeCatalogRepository,
    shared::{spawn_background_task, BackgroundTaskSpawner, EventSink, InFlightRegistry},
    workflow_catalog::WorkflowCatalogRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    errors::{invalid_state, lifecycle_journal_error, workspace_not_found, WorkspaceError},
    events::WorkspaceEvent,
    registry::WorkspaceRuntimeRegistry,
    runtime::{
        CleanupWorkspaceResponse, CreateRunpodWorkspaceRequest, DeleteWorkspaceResponse,
        ProvisionWorkspaceResponse, WorkspaceRuntimeContext,
    },
};

pub type LifecycleOperationRegistry = InFlightRegistry<String>;

#[derive(Clone)]
pub struct WorkspaceService<W, L, WC, RC>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    WC: WorkflowCatalogRepository,
    RC: RuntimeCatalogRepository,
{
    workspace_catalog: Arc<W>,
    lifecycle_journal: Arc<L>,
    workflow_catalog: WC,
    runtime_catalog: RC,
    runtime_registry: WorkspaceRuntimeRegistry,
    lifecycle_operation_registry: LifecycleOperationRegistry,
    event_sink: Arc<dyn EventSink<WorkspaceEvent>>,
    task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

pub struct WorkspaceServiceDependencies<W, L, WC, RC>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    WC: WorkflowCatalogRepository,
    RC: RuntimeCatalogRepository,
{
    pub workspace_catalog: Arc<W>,
    pub lifecycle_journal: Arc<L>,
    pub workflow_catalog: WC,
    pub runtime_catalog: RC,
    pub runtime_registry: WorkspaceRuntimeRegistry,
    pub lifecycle_operation_registry: LifecycleOperationRegistry,
    pub event_sink: Arc<dyn EventSink<WorkspaceEvent>>,
    pub task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

impl<W, L, WC, RC> WorkspaceService<W, L, WC, RC>
where
    W: WorkspaceCatalogRepository + 'static,
    L: LifecycleJournalRepository + 'static,
    WC: WorkflowCatalogRepository + Clone + Send + Sync + 'static,
    RC: RuntimeCatalogRepository + Clone + Send + Sync + 'static,
{
    pub fn new(dependencies: WorkspaceServiceDependencies<W, L, WC, RC>) -> Self {
        Self {
            workspace_catalog: dependencies.workspace_catalog,
            lifecycle_journal: dependencies.lifecycle_journal,
            workflow_catalog: dependencies.workflow_catalog,
            runtime_catalog: dependencies.runtime_catalog,
            runtime_registry: dependencies.runtime_registry,
            lifecycle_operation_registry: dependencies.lifecycle_operation_registry,
            event_sink: dependencies.event_sink,
            task_spawner: dependencies.task_spawner,
        }
    }

    pub async fn create_runpod_workspace(
        &self,
        request: CreateRunpodWorkspaceRequest,
    ) -> Result<Workspace, WorkspaceError> {
        if request.workspace_id.trim().is_empty() {
            return Err(invalid_state("workspace id is required"));
        }

        let workflow_catalog = self.workflow_catalog.get_workflow_catalog()?;
        let _ = &self.runtime_catalog;
        let workflow = workflow_catalog
            .workflow_presets
            .iter()
            .find(|preset| preset.id == request.workflow_preset_id)
            .ok_or_else(|| invalid_state("workflow preset was not found"))?;
        let revision = workflow
            .revisions
            .last()
            .ok_or_else(|| invalid_state("workflow preset was not found"))?;
        if request.placement.volume_size_gb < revision.required_volume_size_gb {
            return Err(invalid_state(
                "requested volume is smaller than the workflow requires",
            ));
        }

        let workspace = Workspace {
            id: request.workspace_id,
            workflow: WorkflowReference {
                id: workflow.id.clone(),
                version: revision.version.clone(),
            },
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntimeDomain::Runpod(RunpodRuntime {
                placement: request.placement,
                resources: empty_runpod_resources(),
            }),
        };
        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        self.event_sink.emit(WorkspaceEvent::WorkspaceChanged {
            workspace_id: workspace.id.clone(),
            workspace: Box::new(workspace.clone()),
        });
        Ok(workspace)
    }

    async fn start_lifecycle_operation(
        &self,
        workspace_id: &str,
    ) -> Result<(Workspace, LifecycleOperation), WorkspaceError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        let workspace_id = workspace.id.clone();

        if self
            .lifecycle_journal
            .find_running_by_workspace(&workspace_id)
            .await
            .map_err(lifecycle_journal_error)?
            .is_some()
        {
            return Err(WorkspaceError::LifecycleOperationAlreadyRunning { workspace_id });
        }

        let operation = self
            .lifecycle_journal
            .create_operation(&workspace_id)
            .await
            .map_err(map_operation_start_error(&workspace_id))?;
        self.event_sink
            .emit(WorkspaceEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                diagnostic_id: None,
                operation: operation.clone(),
            });

        Ok((workspace, operation))
    }

    pub async fn provision_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<ProvisionWorkspaceResponse, WorkspaceError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        if workspace.state != WorkspaceState::NotProvisioned {
            return Err(invalid_state("workspace is not ready to provision"));
        }
        let WorkspaceRuntimeDomain::Runpod(runtime) = &workspace.runtime;
        if !runpod_resources_are_empty(&runtime.resources) {
            return Err(invalid_state("workspace already has runpod resources"));
        }
        let (workspace, operation) = self.start_lifecycle_operation(workspace_id).await?;

        let runtime = self.runtime_registry.runtime_for(&workspace.runtime)?;
        let context = self.runtime_context();
        let runner_operation = operation.clone();
        let runner_workspace = workspace.clone();
        self.spawn_lifecycle_runner(operation.clone(), workspace.clone(), async move {
            runtime
                .provision(context, runner_operation, runner_workspace)
                .await
        });

        Ok(ProvisionWorkspaceResponse {
            workspace,
            operation,
        })
    }

    pub async fn cleanup_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<CleanupWorkspaceResponse, WorkspaceError> {
        let (workspace, operation) = self.start_lifecycle_operation(workspace_id).await?;
        let runtime = self.runtime_registry.runtime_for(&workspace.runtime)?;
        let context = self.runtime_context();
        let runner_operation = operation.clone();
        let runner_workspace = workspace.clone();
        self.spawn_lifecycle_runner(operation.clone(), workspace.clone(), async move {
            runtime
                .cleanup(context, runner_operation, runner_workspace)
                .await
        });

        Ok(CleanupWorkspaceResponse {
            workspace,
            operation,
        })
    }

    fn spawn_lifecycle_runner<F>(
        &self,
        operation: LifecycleOperation,
        workspace: Workspace,
        lifecycle: F,
    ) where
        F: std::future::Future<Output = Result<Workspace, WorkspaceError>> + Send + 'static,
    {
        if !self
            .lifecycle_operation_registry
            .try_register(&operation.operation_id)
        {
            return;
        }

        let lifecycle_operation_registry = self.lifecycle_operation_registry.clone();
        let workspace_catalog = self.workspace_catalog.clone();
        let context = self.runtime_context();
        spawn_background_task(self.task_spawner.as_ref(), async move {
            match lifecycle.await {
                Ok(_) => {
                    let mut completed = operation.clone();
                    completed.state = LifecycleOperationState::Completed;
                    let _ = context.persist_operation(completed).await;
                }
                Err(_) => {
                    if let Ok(Some(mut persisted_workspace)) =
                        workspace_catalog.find_workspace_by_id(&workspace.id).await
                    {
                        persisted_workspace.state = WorkspaceState::Invalid;
                        let _ = context.persist_workspace(persisted_workspace).await;
                    }

                    let mut failed = operation.clone();
                    failed.state = LifecycleOperationState::Failed;
                    let _ = context.persist_operation(failed).await;
                }
            }

            lifecycle_operation_registry.complete(&operation.operation_id);
        });
    }

    pub async fn delete_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<DeleteWorkspaceResponse, WorkspaceError> {
        match self.find_workspace(workspace_id).await? {
            Some(workspace) => {
                let runtime = self.runtime_registry.runtime_for(&workspace.runtime)?;
                runtime.delete(self.runtime_context(), workspace).await?;
            }
            None => {
                self.lifecycle_journal
                    .delete_for_workspace(&workspace_id.to_string())
                    .await
                    .map_err(lifecycle_journal_error)?;
                self.event_sink.emit(WorkspaceEvent::WorkspaceDeleted {
                    workspace_id: workspace_id.to_string(),
                });
            }
        }

        Ok(DeleteWorkspaceResponse {
            workspace_id: workspace_id.to_string(),
        })
    }

    pub async fn get_running_lifecycle_operations(
        &self,
    ) -> Result<Vec<LifecycleOperation>, WorkspaceError> {
        self.lifecycle_journal
            .list_running()
            .await
            .map_err(lifecycle_journal_error)
    }

    pub async fn get_latest_lifecycle_operation(
        &self,
        workspace_id: &str,
    ) -> Result<Option<LifecycleOperation>, WorkspaceError> {
        self.lifecycle_journal
            .latest_for_workspace(&workspace_id.to_string())
            .await
            .map_err(lifecycle_journal_error)
    }

    pub async fn find_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<Workspace>, WorkspaceError> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(WorkspaceError::from)
    }

    pub async fn mark_running_operations_stale(&self) -> Result<(), WorkspaceError> {
        for operation in self.get_running_lifecycle_operations().await? {
            if let Some(mut workspace) = self.find_workspace(&operation.workspace_id).await? {
                workspace.state = WorkspaceState::Invalid;
                self.runtime_context().persist_workspace(workspace).await?;
            }

            let mut stale = operation;
            stale.state = LifecycleOperationState::Stale;
            self.runtime_context().persist_operation(stale).await?;
        }

        Ok(())
    }

    fn runtime_context(&self) -> WorkspaceRuntimeContext<'static> {
        WorkspaceRuntimeContext::new(
            self.workspace_catalog.clone(),
            self.lifecycle_journal.clone(),
            self.event_sink.clone(),
        )
    }

    async fn load_workspace_required(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, WorkspaceError> {
        self.find_workspace(workspace_id)
            .await?
            .ok_or_else(|| workspace_not_found(workspace_id))
    }
}

fn map_operation_start_error(
    workspace_id: &str,
) -> impl FnOnce(LifecycleJournalError) -> WorkspaceError + '_ {
    move |error| match error {
        LifecycleJournalError::RunningOperationExists => {
            WorkspaceError::LifecycleOperationAlreadyRunning {
                workspace_id: workspace_id.to_string(),
            }
        }
        other => lifecycle_journal_error(other),
    }
}

fn empty_runpod_resources() -> RunpodResources {
    RunpodResources {
        network_volume_id: None,
        provisioner_pod_id: None,
        endpoint_id: None,
        template_id: None,
    }
}

fn runpod_resources_are_empty(resources: &RunpodResources) -> bool {
    resources.network_volume_id.is_none()
        && resources.provisioner_pod_id.is_none()
        && resources.endpoint_id.is_none()
        && resources.template_id.is_none()
}

#[cfg(test)]
impl<W, L, WC, RC> WorkspaceService<W, L, WC, RC>
where
    W: WorkspaceCatalogRepository + 'static,
    L: LifecycleJournalRepository + 'static,
    WC: WorkflowCatalogRepository + Clone + Send + Sync + 'static,
    RC: RuntimeCatalogRepository + Clone + Send + Sync + 'static,
{
    pub async fn insert_workspace_for_test(&self, workspace: Workspace) {
        self.workspace_catalog
            .insert_workspace(&workspace)
            .await
            .expect("workspace insert should succeed");
    }

    pub async fn create_running_operation_for_test(
        &self,
        workspace_id: &str,
        payload: Option<crate::domain::lifecycle_operation::LifecycleOperationPayload>,
    ) {
        let mut operation = self
            .lifecycle_journal
            .create_operation(&workspace_id.to_string())
            .await
            .expect("operation create should succeed");
        operation.payload = payload;
        self.lifecycle_journal
            .update_operation(&operation)
            .await
            .expect("operation update should succeed");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{lifecycle_operation::LifecycleOperationState, workspace::WorkspaceState},
        workspace::test_support::{
            draft_create_request, service_with_fake_runtime, workspace_with_runpod,
        },
    };

    #[tokio::test]
    async fn provision_creates_running_operation_with_null_payload() {
        let service = service_with_fake_runtime();
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        let response = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision scheduled");

        assert_eq!(response.operation.state, LifecycleOperationState::Running);
        assert_eq!(response.operation.payload, None);
    }

    #[tokio::test]
    async fn delete_workspace_does_not_create_lifecycle_operation() {
        let service = service_with_fake_runtime();
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;

        let response = service
            .delete_workspace("workspace-1")
            .await
            .expect("delete");

        assert_eq!(response.workspace_id, "workspace-1");
        assert_eq!(
            service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("latest"),
            None
        );
    }

    #[tokio::test]
    async fn stale_running_operation_marks_workspace_invalid() {
        let service = service_with_fake_runtime();
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;
        service
            .create_running_operation_for_test("workspace-1", None)
            .await;

        service
            .mark_running_operations_stale()
            .await
            .expect("stale recovery");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("find")
            .expect("workspace exists");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
    }

    #[tokio::test]
    async fn provision_rejects_invalid_workspace_without_creating_operation() {
        let service = service_with_fake_runtime();
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;

        service
            .provision_workspace("workspace-1")
            .await
            .expect_err("provision should fail");

        assert_eq!(
            service
                .get_running_lifecycle_operations()
                .await
                .expect("operations"),
            Vec::new()
        );
    }
}
