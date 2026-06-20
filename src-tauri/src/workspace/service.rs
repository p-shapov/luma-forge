use std::sync::Arc;

use tracing::Instrument;

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationState},
        runpod::{RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceRuntime as WorkspaceRuntimeDomain, WorkspaceState},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    shared::{
        leaf_error_message, new_trace_id, spawn_background_task, BackgroundTaskSpawner, EventSink,
        InFlightRegistry,
    },
    workflow_catalog::WorkflowCatalogRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    errors::{invalid_state, workspace_not_found, WorkspaceError},
    events::WorkspaceEvent,
    runtime::{
        CleanupWorkspaceResponse, CreateRunpodWorkspaceRequest, DeleteWorkspaceResponse,
        ProvisionWorkspaceResponse, WorkspaceRuntime, WorkspaceRuntimeContext,
    },
};

pub type LifecycleOperationRegistry = InFlightRegistry<String>;

#[derive(Clone)]
pub struct WorkspaceService<W, L, WC>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    WC: WorkflowCatalogRepository,
{
    workspace_catalog: Arc<W>,
    lifecycle_journal: Arc<L>,
    workflow_catalog: WC,
    runtime: Arc<dyn WorkspaceRuntime>,
    lifecycle_operation_registry: LifecycleOperationRegistry,
    event_sink: Arc<dyn EventSink<WorkspaceEvent>>,
    task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

pub struct WorkspaceServiceDependencies<W, L, WC>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
    WC: WorkflowCatalogRepository,
{
    pub workspace_catalog: Arc<W>,
    pub lifecycle_journal: Arc<L>,
    pub workflow_catalog: WC,
    pub runtime: Arc<dyn WorkspaceRuntime>,
    pub lifecycle_operation_registry: LifecycleOperationRegistry,
    pub event_sink: Arc<dyn EventSink<WorkspaceEvent>>,
    pub task_spawner: Arc<dyn BackgroundTaskSpawner>,
}

impl<W, L, WC> WorkspaceService<W, L, WC>
where
    W: WorkspaceCatalogRepository + 'static,
    L: LifecycleJournalRepository + 'static,
    WC: WorkflowCatalogRepository + Clone + Send + Sync + 'static,
{
    pub fn new(dependencies: WorkspaceServiceDependencies<W, L, WC>) -> Self {
        Self {
            workspace_catalog: dependencies.workspace_catalog,
            lifecycle_journal: dependencies.lifecycle_journal,
            workflow_catalog: dependencies.workflow_catalog,
            runtime: dependencies.runtime,
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
                resources: RunpodResources::default(),
            }),
        };
        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        self.event_sink.emit(WorkspaceEvent::WorkspaceChanged {
            workspace_id: workspace.id.clone(),
            workspace: workspace.clone(),
        });
        Ok(workspace)
    }

    async fn start_lifecycle_operation(
        &self,
        workspace_id: &str,
        trace_id: String,
    ) -> Result<(Workspace, LifecycleOperation, String), WorkspaceError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        let workspace_id = workspace.id.clone();

        let operation = self
            .lifecycle_journal
            .create_operation(&workspace_id)
            .await
            .map_err(map_operation_start_error(&workspace_id))?;
        self.event_sink
            .emit(WorkspaceEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                trace_id: trace_id.clone(),
                operation: operation.clone(),
            });

        Ok((workspace, operation, trace_id))
    }

    pub async fn provision_workspace(
        &self,
        workspace_id: &str,
        trace_id: String,
    ) -> Result<ProvisionWorkspaceResponse, WorkspaceError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        if workspace.state != WorkspaceState::NotProvisioned {
            return Err(invalid_state("workspace is not ready to provision"));
        }
        let WorkspaceRuntimeDomain::Runpod(runtime) = &workspace.runtime;
        if runtime.resources != RunpodResources::default() {
            return Err(invalid_state("workspace already has runpod resources"));
        }
        let (workspace, operation, trace_id) = self
            .start_lifecycle_operation(workspace_id, trace_id)
            .await?;

        let runtime = self.runtime.clone();
        let context = self.runtime_context(trace_id.clone());
        let runner_operation = operation.clone();
        let runner_workspace = workspace.clone();
        self.spawn_lifecycle_runner(operation.clone(), workspace.clone(), trace_id, async move {
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
        trace_id: String,
    ) -> Result<CleanupWorkspaceResponse, WorkspaceError> {
        let (workspace, operation, trace_id) = self
            .start_lifecycle_operation(workspace_id, trace_id)
            .await?;
        let runtime = self.runtime.clone();
        let context = self.runtime_context(trace_id.clone());
        let runner_operation = operation.clone();
        let runner_workspace = workspace.clone();
        self.spawn_lifecycle_runner(operation.clone(), workspace.clone(), trace_id, async move {
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
        trace_id: String,
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
        let lifecycle_journal = self.lifecycle_journal.clone();
        let workspace_catalog = self.workspace_catalog.clone();
        let context = self.runtime_context(trace_id);
        let lifecycle_span = tracing::info_span!(
            "workspace_lifecycle",
            trace_id = %context.trace_id(),
            workspace_id = %workspace.id,
            operation_id = %operation.operation_id
        );
        spawn_background_task(
            self.task_spawner.as_ref(),
            async move {
                let persisted_terminal_state = match lifecycle.await {
                    Ok(_) => {
                        let mut completed = load_terminal_operation(
                            lifecycle_journal.as_ref(),
                            &operation,
                            &workspace.id,
                        )
                        .await
                        .unwrap_or_else(|_| operation.clone());
                        completed.state = LifecycleOperationState::Completed;
                        context.persist_operation(completed).await.is_ok()
                    }
                    Err(error) => {
                        let error_message = leaf_error_message(&error);
                        tracing::error!(
                            error = ?error_message,
                            "workspace lifecycle operation failed"
                        );

                        if let Ok(Some(mut persisted_workspace)) =
                            workspace_catalog.find_workspace_by_id(&workspace.id).await
                        {
                            persisted_workspace.state = WorkspaceState::Invalid;
                            let _ = context.persist_workspace(persisted_workspace).await;
                        }

                        let mut failed = load_terminal_operation(
                            lifecycle_journal.as_ref(),
                            &operation,
                            &workspace.id,
                        )
                        .await
                        .unwrap_or_else(|_| operation.clone());
                        failed.state = LifecycleOperationState::Failed;
                        context.persist_operation(failed).await.is_ok()
                    }
                };

                if persisted_terminal_state {
                    lifecycle_operation_registry.complete(&operation.operation_id);
                }
            }
            .instrument(lifecycle_span),
        );
    }

    pub async fn delete_workspace(
        &self,
        workspace_id: &str,
        trace_id: String,
    ) -> Result<DeleteWorkspaceResponse, WorkspaceError> {
        let (workspace, operation, trace_id) = self
            .start_lifecycle_operation(workspace_id, trace_id)
            .await?;
        let runtime = self.runtime.clone();
        let context = self.runtime_context(trace_id.clone());
        let runner_operation = operation.clone();
        let runner_workspace = workspace.clone();
        self.spawn_lifecycle_runner(operation, workspace, trace_id, async move {
            runtime
                .delete(context, runner_operation, runner_workspace)
                .await
        });

        Ok(DeleteWorkspaceResponse {
            workspace_id: workspace_id.to_string(),
        })
    }

    pub async fn mark_running_operations_stale(&self) -> Result<(), WorkspaceError> {
        for operation in self.lifecycle_journal.list_running().await? {
            if let Some(mut workspace) = self
                .workspace_catalog
                .find_workspace_by_id(&operation.workspace_id)
                .await?
            {
                workspace.state = WorkspaceState::Invalid;
                self.runtime_context(new_trace_id())
                    .persist_workspace(workspace)
                    .await?;
            }

            let mut stale = operation;
            stale.state = LifecycleOperationState::Stale;
            self.runtime_context(new_trace_id())
                .persist_operation(stale)
                .await?;
        }

        Ok(())
    }

    fn runtime_context(&self, trace_id: String) -> WorkspaceRuntimeContext<'static> {
        WorkspaceRuntimeContext::new(
            self.workspace_catalog.clone(),
            self.lifecycle_journal.clone(),
            self.event_sink.clone(),
            trace_id,
        )
    }

    async fn load_workspace_required(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, WorkspaceError> {
        self.workspace_catalog
            .find_workspace_by_id(workspace_id)
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
        other => other.into(),
    }
}

async fn load_terminal_operation(
    lifecycle_journal: &dyn LifecycleJournalRepository,
    operation: &LifecycleOperation,
    workspace_id: &str,
) -> Result<LifecycleOperation, LifecycleJournalError> {
    match lifecycle_journal
        .find_running_by_workspace(&workspace_id.to_string())
        .await?
    {
        Some(persisted) if persisted.operation_id == operation.operation_id => Ok(persisted),
        _ => Ok(operation.clone()),
    }
}

#[cfg(test)]
impl<W, L, WC> WorkspaceService<W, L, WC>
where
    W: WorkspaceCatalogRepository + 'static,
    L: LifecycleJournalRepository + 'static,
    WC: WorkflowCatalogRepository + Clone + Send + Sync + 'static,
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
    use std::sync::Arc;
    use std::time::Duration;

    use crate::{
        domain::{
            lifecycle_operation::{LifecycleOperation, LifecycleOperationState},
            workspace::{Workspace, WorkspaceState},
        },
        lifecycle_journal::LifecycleJournalRepository,
        provider::runpod::test_support::{draft_create_request, workspace_with_runpod},
        shared::AppFuture,
        workspace::test_support::{
            service_with_fake_runtime, service_with_runtime, FakeWorkspaceRuntime,
        },
        workspace_catalog::WorkspaceCatalogRepository,
    };

    #[derive(Debug, Clone, Copy)]
    struct FailingCleanupRuntime;

    impl crate::workspace::runtime::WorkspaceRuntime for FailingCleanupRuntime {
        fn provision<'a>(
            &'a self,
            _context: crate::workspace::WorkspaceRuntimeContext<'a>,
            _operation: LifecycleOperation,
            workspace: Workspace,
        ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
            Box::pin(async move { Ok(workspace) })
        }

        fn cleanup<'a>(
            &'a self,
            _context: crate::workspace::WorkspaceRuntimeContext<'a>,
            _operation: LifecycleOperation,
            _workspace: Workspace,
        ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
            Box::pin(async move { Err(crate::workspace::errors::invalid_state("cleanup failed")) })
        }

        fn delete<'a>(
            &'a self,
            context: crate::workspace::WorkspaceRuntimeContext<'a>,
            _operation: LifecycleOperation,
            workspace: Workspace,
        ) -> AppFuture<'a, Result<Workspace, crate::workspace::WorkspaceError>> {
            Box::pin(async move {
                context.delete_workspace(&workspace.id).await?;
                Ok(workspace)
            })
        }
    }

    async fn wait_for_operation_state(
        repositories: &crate::workspace::test_support::TestRepositories,
        workspace_id: &str,
        expected_state: LifecycleOperationState,
    ) -> LifecycleOperation {
        for _ in 0..20 {
            let workspace_id = workspace_id.to_string();
            if let Some(operation) = repositories
                .lifecycle_journal
                .latest_for_workspace(&workspace_id)
                .await
                .expect("latest operation should load")
            {
                if operation.state == expected_state {
                    return operation;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        panic!("operation did not reach expected state");
    }

    #[tokio::test]
    async fn provision_creates_running_operation_with_null_payload() {
        let service = service_with_fake_runtime();
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        let response = service
            .provision_workspace("workspace-1", "trace-test".to_string())
            .await
            .expect("provision scheduled");

        assert_eq!(response.operation.state, LifecycleOperationState::Running);
        assert_eq!(response.operation.payload, None);
    }

    #[tokio::test]
    async fn stale_running_operation_marks_workspace_invalid() {
        let (service, repositories) = service_with_runtime(Arc::new(FakeWorkspaceRuntime));
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

        let workspace = repositories
            .workspace_catalog
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find")
            .expect("workspace exists");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
        let stale = repositories
            .lifecycle_journal
            .latest_for_workspace(&"workspace-1".to_string())
            .await
            .expect("latest")
            .expect("operation exists");
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(stale.payload, None);
    }

    #[tokio::test]
    async fn provision_rejects_invalid_workspace_without_creating_operation() {
        let (service, repositories) = service_with_runtime(Arc::new(FakeWorkspaceRuntime));
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;

        service
            .provision_workspace("workspace-1", "trace-test".to_string())
            .await
            .expect_err("provision should fail");

        assert_eq!(
            repositories
                .lifecycle_journal
                .list_running()
                .await
                .expect("operations"),
            Vec::new()
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_operation_completed_and_clears_in_flight() {
        let (service, repositories) = service_with_runtime(Arc::new(FakeWorkspaceRuntime));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        let response = service
            .provision_workspace("workspace-1", "trace-test".to_string())
            .await
            .expect("provision scheduled");
        let completed = wait_for_operation_state(
            &repositories,
            "workspace-1",
            LifecycleOperationState::Completed,
        )
        .await;

        assert_eq!(completed.operation_id, response.operation.operation_id);
        assert_eq!(completed.payload, None);
        assert!(service
            .lifecycle_operation_registry
            .try_register(&response.operation.operation_id));
    }

    #[tokio::test]
    async fn cleanup_runner_marks_workspace_invalid_operation_failed_and_clears_in_flight() {
        let (service, repositories) = service_with_runtime(Arc::new(FailingCleanupRuntime));
        service
            .insert_workspace_for_test(workspace_with_runpod(
                "workspace-1",
                WorkspaceState::CleanupRequired,
            ))
            .await;

        let response = service
            .cleanup_workspace("workspace-1", "trace-test".to_string())
            .await
            .expect("cleanup scheduled");
        let failed = wait_for_operation_state(
            &repositories,
            "workspace-1",
            LifecycleOperationState::Failed,
        )
        .await;

        assert_eq!(failed.operation_id, response.operation.operation_id);
        let workspace = repositories
            .workspace_catalog
            .find_workspace_by_id("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
        assert!(service
            .lifecycle_operation_registry
            .try_register(&response.operation.operation_id));
    }

    #[tokio::test]
    async fn delete_workspace_rejects_when_lifecycle_operation_is_running() {
        let (service, repositories) = service_with_runtime(Arc::new(FakeWorkspaceRuntime));
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;
        service
            .create_running_operation_for_test("workspace-1", None)
            .await;

        let error = service
            .delete_workspace("workspace-1", "trace-test".to_string())
            .await
            .expect_err("delete should be rejected");

        assert_eq!(
            error,
            crate::workspace::WorkspaceError::LifecycleOperationAlreadyRunning {
                workspace_id: "workspace-1".to_string(),
            }
        );
        assert!(repositories
            .workspace_catalog
            .find_workspace_by_id("workspace-1")
            .await
            .expect("workspace lookup should succeed")
            .is_some());
        assert_eq!(
            repositories
                .lifecycle_journal
                .latest_for_workspace(&"workspace-1".to_string())
                .await
                .expect("latest operation should load")
                .expect("operation should exist")
                .state,
            LifecycleOperationState::Running
        );
    }
}
