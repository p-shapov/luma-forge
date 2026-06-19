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
        let lifecycle_journal = self.lifecycle_journal.clone();
        let workspace_catalog = self.workspace_catalog.clone();
        let context = self.runtime_context();
        spawn_background_task(self.task_spawner.as_ref(), async move {
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
                Err(_) => {
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
        });
    }

    pub async fn delete_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<DeleteWorkspaceResponse, WorkspaceError> {
        if self
            .lifecycle_journal
            .find_running_by_workspace(&workspace_id.to_string())
            .await
            .map_err(lifecycle_journal_error)?
            .is_some()
        {
            return Err(WorkspaceError::LifecycleOperationAlreadyRunning {
                workspace_id: workspace_id.to_string(),
            });
        }

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
    use std::sync::Arc;
    use std::time::Duration;

    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleCleanupPayload, LifecycleOperation, LifecycleOperationPayload,
                LifecycleOperationState, LifecycleProvisionPayload,
            },
            runpod::{
                RunpodCleanupStep, RunpodLifecycleCleanupPayload, RunpodLifecycleProvisionPayload,
                RunpodProvisionStep,
            },
            workspace::WorkspaceState,
        },
        lifecycle_journal::LifecycleJournalError,
        shared::AppFuture,
        workspace::test_support::{
            draft_create_request, service_with_fake_runtime, service_with_runtime,
            workspace_with_runpod, FakeWorkspaceRuntime,
        },
    };

    fn cleanup_payload() -> LifecycleOperationPayload {
        LifecycleOperationPayload::Cleanup(LifecycleCleanupPayload::Runpod(
            RunpodLifecycleCleanupPayload {
                step: Some(RunpodCleanupStep::DeleteEndpoint),
            },
        ))
    }

    fn provision_payload() -> LifecycleOperationPayload {
        LifecycleOperationPayload::Provision(LifecycleProvisionPayload::Runpod(
            RunpodLifecycleProvisionPayload {
                step: Some(RunpodProvisionStep::CreateNetworkVolume),
            },
        ))
    }

    #[derive(Debug, Clone)]
    struct ProgressThenSucceedRuntime {
        payload: LifecycleOperationPayload,
    }

    impl crate::workspace::runtime::WorkspaceRuntime for ProgressThenSucceedRuntime {
        fn provision<'a>(
            &'a self,
            context: crate::workspace::WorkspaceRuntimeContext<'a>,
            mut operation: LifecycleOperation,
            workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<
            'a,
            Result<crate::domain::workspace::Workspace, crate::workspace::WorkspaceError>,
        > {
            let payload = self.payload.clone();
            Box::pin(async move {
                operation.payload = Some(payload);
                context.persist_operation(operation).await?;
                Ok(workspace)
            })
        }

        fn cleanup<'a>(
            &'a self,
            _context: crate::workspace::WorkspaceRuntimeContext<'a>,
            _operation: LifecycleOperation,
            workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<
            'a,
            Result<crate::domain::workspace::Workspace, crate::workspace::WorkspaceError>,
        > {
            Box::pin(async move { Ok(workspace) })
        }

        fn delete<'a>(
            &'a self,
            context: crate::workspace::WorkspaceRuntimeContext<'a>,
            workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<'a, Result<(), crate::workspace::WorkspaceError>> {
            Box::pin(async move { context.delete_workspace(&workspace.id).await })
        }
    }

    #[derive(Debug, Clone)]
    struct ProgressThenFailRuntime {
        payload: LifecycleOperationPayload,
    }

    impl crate::workspace::runtime::WorkspaceRuntime for ProgressThenFailRuntime {
        fn provision<'a>(
            &'a self,
            context: crate::workspace::WorkspaceRuntimeContext<'a>,
            mut operation: LifecycleOperation,
            _workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<
            'a,
            Result<crate::domain::workspace::Workspace, crate::workspace::WorkspaceError>,
        > {
            let payload = self.payload.clone();
            Box::pin(async move {
                operation.payload = Some(payload);
                context.persist_operation(operation).await?;
                Err(crate::workspace::errors::invalid_state("provision failed"))
            })
        }

        fn cleanup<'a>(
            &'a self,
            _context: crate::workspace::WorkspaceRuntimeContext<'a>,
            _operation: LifecycleOperation,
            workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<
            'a,
            Result<crate::domain::workspace::Workspace, crate::workspace::WorkspaceError>,
        > {
            Box::pin(async move { Ok(workspace) })
        }

        fn delete<'a>(
            &'a self,
            context: crate::workspace::WorkspaceRuntimeContext<'a>,
            workspace: crate::domain::workspace::Workspace,
        ) -> AppFuture<'a, Result<(), crate::workspace::WorkspaceError>> {
            Box::pin(async move { context.delete_workspace(&workspace.id).await })
        }
    }

    async fn wait_for_operation_state(
        service: &crate::workspace::WorkspaceService<
            crate::workspace::test_support::InMemoryWorkspaceRepository,
            crate::workspace::test_support::InMemoryLifecycleJournal,
            crate::workflow_catalog::BundledWorkflowCatalogRepository,
            crate::runtime_catalog::BundledRuntimeCatalogRepository,
        >,
        workspace_id: &str,
        expected_state: LifecycleOperationState,
    ) -> crate::domain::lifecycle_operation::LifecycleOperation {
        for _ in 0..20 {
            if let Some(operation) = service
                .get_latest_lifecycle_operation(workspace_id)
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
        let stale = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("latest")
            .expect("operation exists");
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(stale.payload, None);
    }

    #[tokio::test]
    async fn stale_running_operation_retains_existing_payload() {
        let service = service_with_fake_runtime();
        let payload = cleanup_payload();
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;
        service
            .create_running_operation_for_test("workspace-1", Some(payload.clone()))
            .await;

        service
            .mark_running_operations_stale()
            .await
            .expect("stale recovery");

        let stale = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("latest")
            .expect("operation exists");
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(stale.payload, Some(payload));
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

    #[tokio::test]
    async fn provision_runner_marks_operation_completed_and_clears_in_flight() {
        let service = service_with_fake_runtime();
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        let response = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision scheduled");
        let completed =
            wait_for_operation_state(&service, "workspace-1", LifecycleOperationState::Completed)
                .await;

        assert_eq!(completed.operation_id, response.operation.operation_id);
        assert_eq!(completed.payload, None);
        assert!(service
            .lifecycle_operation_registry
            .try_register(&response.operation.operation_id));
    }

    #[tokio::test]
    async fn provision_runner_keeps_progress_payload_when_marking_completed() {
        let payload = provision_payload();
        let (service, _) = service_with_runtime(Arc::new(ProgressThenSucceedRuntime {
            payload: payload.clone(),
        }));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        service
            .provision_workspace("workspace-1")
            .await
            .expect("provision scheduled");
        let completed =
            wait_for_operation_state(&service, "workspace-1", LifecycleOperationState::Completed)
                .await;

        assert_eq!(completed.payload, Some(payload));
    }

    #[tokio::test]
    async fn cleanup_runner_marks_workspace_invalid_operation_failed_and_clears_in_flight() {
        let service = service_with_fake_runtime();
        service
            .insert_workspace_for_test(workspace_with_runpod(
                "workspace-1",
                WorkspaceState::CleanupRequired,
            ))
            .await;

        let (_, operation) = service
            .start_lifecycle_operation("workspace-1")
            .await
            .expect("operation should start");
        let operation_id = operation.operation_id.clone();
        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        service.spawn_lifecycle_runner(operation, workspace, async {
            Err::<crate::domain::workspace::Workspace, _>(crate::workspace::errors::invalid_state(
                "cleanup failed",
            ))
        });
        let failed =
            wait_for_operation_state(&service, "workspace-1", LifecycleOperationState::Failed)
                .await;

        assert_eq!(failed.operation_id, operation_id);
        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
        assert!(service
            .lifecycle_operation_registry
            .try_register(&operation_id));
    }

    #[tokio::test]
    async fn provision_runner_keeps_progress_payload_when_marking_failed() {
        let payload = provision_payload();
        let (service, _) = service_with_runtime(Arc::new(ProgressThenFailRuntime {
            payload: payload.clone(),
        }));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");

        service
            .provision_workspace("workspace-1")
            .await
            .expect("provision scheduled");
        let failed =
            wait_for_operation_state(&service, "workspace-1", LifecycleOperationState::Failed)
                .await;

        assert_eq!(failed.payload, Some(payload));
    }

    #[tokio::test]
    async fn runner_keeps_in_flight_when_terminal_operation_persistence_fails() {
        let (service, repositories) =
            service_with_runtime(std::sync::Arc::new(FakeWorkspaceRuntime));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace");
        repositories.lifecycle_journal.fail_mark_state_once(
            LifecycleJournalError::StorageUnavailable {
                message: "write failed".to_string(),
            },
        );

        let response = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision scheduled");

        tokio::time::sleep(Duration::from_millis(50)).await;

        let operation = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("latest")
            .expect("operation exists");
        assert_eq!(operation.state, LifecycleOperationState::Running);
        assert!(!service
            .lifecycle_operation_registry
            .try_register(&response.operation.operation_id));
    }

    #[tokio::test]
    async fn delete_workspace_rejects_when_lifecycle_operation_is_running() {
        let service = service_with_fake_runtime();
        service
            .insert_workspace_for_test(workspace_with_runpod("workspace-1", WorkspaceState::Ready))
            .await;
        service
            .create_running_operation_for_test("workspace-1", Some(cleanup_payload()))
            .await;

        let error = service
            .delete_workspace("workspace-1")
            .await
            .expect_err("delete should be rejected");

        assert_eq!(
            error,
            crate::workspace::WorkspaceError::LifecycleOperationAlreadyRunning {
                workspace_id: "workspace-1".to_string(),
            }
        );
        assert!(service
            .find_workspace("workspace-1")
            .await
            .expect("workspace lookup should succeed")
            .is_some());
        assert_eq!(
            service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("latest operation should load")
                .expect("operation should exist")
                .state,
            LifecycleOperationState::Running
        );
    }
}
