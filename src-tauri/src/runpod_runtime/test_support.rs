use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use time::OffsetDateTime;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationId, LifecycleOperationPayload,
            LifecycleOperationState,
        },
        runpod::{
            RunpodDatacenterPlacementOption, RunpodGpuPlacementOption, RunpodPlacementOptions,
            RunpodPlacementPlan,
        },
        workspace::{Workspace, WorkspaceCatalog, WorkspaceId},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    runpod_runtime::provider::RunpodProvisionerStatus,
    runtime_catalog::{BundledRuntimeCatalogRepository, RuntimeCatalogRepository},
    shared::{AppFuture, BackgroundTask, BackgroundTaskSpawner, NoopEventSink},
    workflow_catalog::{BundledWorkflowCatalogRepository, WorkflowCatalogRepository},
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

use super::{
    errors::RunpodRuntimeError,
    lifecycle,
    provider::{
        CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
        CreateRunpodServerlessTemplateParams, RunpodRuntimeClient, StartRunpodProvisionerPodParams,
    },
    service::{
        CreateRunpodWorkspaceRequest, RunpodRuntimeService, RunpodRuntimeServiceDependencies,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestBackgroundTaskSpawner;

impl BackgroundTaskSpawner for TestBackgroundTaskSpawner {
    fn spawn(&self, task: BackgroundTask) {
        tokio::spawn(task);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoopBackgroundTaskSpawner;

impl BackgroundTaskSpawner for NoopBackgroundTaskSpawner {
    fn spawn(&self, _task: BackgroundTask) {}
}

pub(crate) trait ManualLifecycleRunnerExt {
    fn run_provision_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn run_cleanup_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn run_delete_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;
}

impl<W, L, WC, RC> ManualLifecycleRunnerExt for RunpodRuntimeService<W, L, WC, RC>
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: LifecycleJournalRepository + Clone + Send + Sync + 'static,
    WC: WorkflowCatalogRepository + Clone + Send + Sync + 'static,
    RC: RuntimeCatalogRepository + Clone + Send + Sync + 'static,
{
    fn run_provision_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::provision::run_once(
                &operation_id,
                lifecycle::provision::RunpodProvisionLifecycleContext {
                    workspace_catalog: &context.workspace_catalog,
                    lifecycle_journal: &context.lifecycle_journal,
                    workflow_catalog: &context.workflow_catalog,
                    runtime_catalog: &context.runtime_catalog,
                    runpod_client: context.runpod_client.as_ref(),
                    event_sink: &context.event_sink,
                    provisioner_poll_interval: Duration::ZERO,
                },
            )
            .await;
            context.lifecycle_operation_registry.complete(&operation_id);
            result
        })
    }

    fn run_cleanup_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::cleanup::run_once(
                &operation_id,
                &context.workspace_catalog,
                &context.lifecycle_journal,
                context.runpod_client.as_ref(),
                &context.event_sink,
            )
            .await;
            context.lifecycle_operation_registry.complete(&operation_id);
            result
        })
    }

    fn run_delete_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::delete::run_once(
                &operation_id,
                &context.workspace_catalog,
                &context.lifecycle_journal,
                context.runpod_client.as_ref(),
                &context.event_sink,
            )
            .await
            .map(|_| ());
            context.lifecycle_operation_registry.complete(&operation_id);
            result
        })
    }
}

#[derive(Default)]
pub(crate) struct RunpodClientState {
    pub(crate) calls: Vec<&'static str>,
    pub(crate) provisioner_image_refs: Vec<String>,
    pub(crate) endpoint_image_refs: Vec<String>,
    pub(crate) placement_options_result: Option<Result<RunpodPlacementOptions, RunpodRuntimeError>>,
    pub(crate) provisioner_status_results: Vec<RunpodProvisionerStatus>,
    pub(crate) create_network_volume_error: Option<RunpodRuntimeError>,
    pub(crate) start_provisioner_pod_error: Option<RunpodRuntimeError>,
    pub(crate) terminate_provisioner_pod_error: Option<RunpodRuntimeError>,
    pub(crate) get_provisioner_status_error: Option<RunpodRuntimeError>,
    pub(crate) create_serverless_template_error: Option<RunpodRuntimeError>,
    pub(crate) create_serverless_endpoint_error: Option<RunpodRuntimeError>,
    pub(crate) delete_serverless_endpoint_error: Option<RunpodRuntimeError>,
    pub(crate) delete_template_error: Option<RunpodRuntimeError>,
    pub(crate) delete_network_volume_error: Option<RunpodRuntimeError>,
}

#[derive(Clone, Default)]
pub(crate) struct WorkspaceRepositoryState {
    pub(crate) delete_workspace_error: Option<WorkspaceCatalogError>,
}

pub(crate) fn placement_options() -> RunpodPlacementOptions {
    RunpodPlacementOptions {
        max_volume_size_gb: Some(10),
        datacenters: vec![RunpodDatacenterPlacementOption {
            id: "dc".to_string(),
            name: "Datacenter".to_string(),
            gpu_options: vec![RunpodGpuPlacementOption {
                id: "gpu".to_string(),
                name: "GPU".to_string(),
                vram_gb: 24,
            }],
        }],
    }
}

struct FakeRunpodRuntimeClient {
    state: Arc<Mutex<RunpodClientState>>,
}

impl FakeRunpodRuntimeClient {
    fn new(state: Arc<Mutex<RunpodClientState>>) -> Self {
        Self { state }
    }
}

impl RunpodRuntimeClient for FakeRunpodRuntimeClient {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("placement_options");

            state
                .placement_options_result
                .clone()
                .unwrap_or_else(|| Ok(placement_options()))
        })
    }

    fn create_network_volume<'a>(
        &'a self,
        _params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("create_network_volume");
            if let Some(error) = state.create_network_volume_error.clone() {
                return Err(error);
            }

            Ok("volume".to_string())
        })
    }

    fn delete_network_volume<'a>(
        &'a self,
        _network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("delete_network_volume");
            if let Some(error) = state.delete_network_volume_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("start_provisioner_pod");
            state
                .provisioner_image_refs
                .push(params.provisioner_image_ref);
            if let Some(error) = state.start_provisioner_pod_error.clone() {
                return Err(error);
            }

            Ok("provisioner".to_string())
        })
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        _provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("terminate_provisioner_pod");
            if let Some(error) = state.terminate_provisioner_pod_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        _workspace_id: &'a str,
        _provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("get_provisioner_status");
            if let Some(error) = state.get_provisioner_status_error.clone() {
                return Err(error);
            }

            if state.provisioner_status_results.is_empty() {
                Ok(RunpodProvisionerStatus::Pending)
            } else {
                Ok(state.provisioner_status_results.remove(0))
            }
        })
    }

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("create_serverless_template");
            state.endpoint_image_refs.push(params.endpoint_image_ref);
            if let Some(error) = state.create_serverless_template_error.clone() {
                return Err(error);
            }

            Ok("template".to_string())
        })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        _params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("create_serverless_endpoint");
            if let Some(error) = state.create_serverless_endpoint_error.clone() {
                return Err(error);
            }

            Ok("endpoint".to_string())
        })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        _endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("delete_serverless_endpoint");
            if let Some(error) = state.delete_serverless_endpoint_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }

    fn delete_template<'a>(
        &'a self,
        _template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("delete_template");
            if let Some(error) = state.delete_template_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct InMemoryWorkspaceRepository {
    workspaces: Arc<Mutex<HashMap<String, Workspace>>>,
    state: Arc<Mutex<WorkspaceRepositoryState>>,
}

impl InMemoryWorkspaceRepository {
    pub(crate) fn with_state(state: Arc<Mutex<WorkspaceRepositoryState>>) -> Self {
        Self {
            workspaces: Arc::new(Mutex::new(HashMap::new())),
            state,
        }
    }
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
            if let Some(error) = self
                .state
                .lock()
                .expect("workspace state lock should succeed")
                .delete_workspace_error
                .clone()
            {
                return Err(error);
            }

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
pub(crate) struct InMemoryLifecycleJournalRepository {
    operations: Arc<Mutex<HashMap<String, LifecycleOperation>>>,
}

impl LifecycleJournalRepository for InMemoryLifecycleJournalRepository {
    fn create_operation<'a>(
        &'a self,
        workspace_id: &'a WorkspaceId,
        payload: &'a LifecycleOperationPayload,
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
                payload: payload.clone(),
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
            let mut operations = self
                .operations
                .lock()
                .expect("operation lock should succeed")
                .values()
                .filter(|operation| operation.state == LifecycleOperationState::Running)
                .cloned()
                .collect::<Vec<_>>();
            operations.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.operation_id.cmp(&right.operation_id))
            });
            Ok(operations)
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
        payload: &'a LifecycleOperationPayload,
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
            operation.payload = payload.clone();
            operation.updated_at = OffsetDateTime::now_utc();
            if state != LifecycleOperationState::Running {
                operation.finished_at = Some(operation.updated_at);
            }
            Ok(operation.clone())
        })
    }
}

pub(crate) fn draft_create_request(workspace_id: &str) -> CreateRunpodWorkspaceRequest {
    CreateRunpodWorkspaceRequest {
        workspace_id: workspace_id.to_string(),
        workflow_preset_id: "comfyui-hidream-o1-dev".to_string(),
        placement: placement_plan(),
    }
}

pub(crate) fn service_with_state(
    state: Arc<Mutex<RunpodClientState>>,
) -> RunpodRuntimeService<
    InMemoryWorkspaceRepository,
    InMemoryLifecycleJournalRepository,
    BundledWorkflowCatalogRepository,
    BundledRuntimeCatalogRepository,
> {
    RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
        workspace_catalog: InMemoryWorkspaceRepository::default(),
        lifecycle_journal: InMemoryLifecycleJournalRepository::default(),
        workflow_catalog: BundledWorkflowCatalogRepository::new(),
        runtime_catalog: BundledRuntimeCatalogRepository::new(),
        runpod_client: Arc::new(FakeRunpodRuntimeClient::new(state)),
        event_sink: Arc::new(NoopEventSink::new()),
        task_spawner: Arc::new(TestBackgroundTaskSpawner),
    })
}

pub(crate) fn service_without_lifecycle_spawning(
    state: Arc<Mutex<RunpodClientState>>,
) -> RunpodRuntimeService<
    InMemoryWorkspaceRepository,
    InMemoryLifecycleJournalRepository,
    BundledWorkflowCatalogRepository,
    BundledRuntimeCatalogRepository,
> {
    RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
        workspace_catalog: InMemoryWorkspaceRepository::default(),
        lifecycle_journal: InMemoryLifecycleJournalRepository::default(),
        workflow_catalog: BundledWorkflowCatalogRepository::new(),
        runtime_catalog: BundledRuntimeCatalogRepository::new(),
        runpod_client: Arc::new(FakeRunpodRuntimeClient::new(state)),
        event_sink: Arc::new(NoopEventSink::new()),
        task_spawner: Arc::new(NoopBackgroundTaskSpawner),
    })
}

pub(crate) fn service_with_state_and_workspace_catalog(
    client_state: Arc<Mutex<RunpodClientState>>,
    workspace_catalog: InMemoryWorkspaceRepository,
) -> RunpodRuntimeService<
    InMemoryWorkspaceRepository,
    InMemoryLifecycleJournalRepository,
    BundledWorkflowCatalogRepository,
    BundledRuntimeCatalogRepository,
> {
    RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
        workspace_catalog,
        lifecycle_journal: InMemoryLifecycleJournalRepository::default(),
        workflow_catalog: BundledWorkflowCatalogRepository::new(),
        runtime_catalog: BundledRuntimeCatalogRepository::new(),
        runpod_client: Arc::new(FakeRunpodRuntimeClient::new(client_state)),
        event_sink: Arc::new(NoopEventSink::new()),
        task_spawner: Arc::new(TestBackgroundTaskSpawner),
    })
}

fn placement_plan() -> RunpodPlacementPlan {
    RunpodPlacementPlan {
        data_center_id: "dc".to_string(),
        gpu_type_id: "gpu".to_string(),
        volume_size_gb: 19,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::runpod::RunpodLifecycleOperationPayload;

    fn provision_payload() -> LifecycleOperationPayload {
        LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision { step: None })
    }

    #[tokio::test]
    async fn update_operation_returns_not_found_for_missing_operation() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let operation = LifecycleOperation {
            operation_id: "missing-operation".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Running,
            payload: provision_payload(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        };

        let error = repository
            .update_operation(&operation)
            .await
            .expect_err("missing operation should not be upserted");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[tokio::test]
    async fn mark_state_returns_not_found_for_terminal_operation() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        let operation = repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("operation should be created");
        repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Completed,
                &payload,
            )
            .await
            .expect("operation should complete");

        let error = repository
            .mark_state(
                &operation.operation_id,
                LifecycleOperationState::Stale,
                &payload,
            )
            .await
            .expect_err("terminal operation should not be marked again");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[tokio::test]
    async fn list_running_returns_created_at_then_operation_id_order() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        let second = repository
            .create_operation(&"workspace-2".to_string(), &payload)
            .await
            .expect("operation should be created");
        let first = LifecycleOperation {
            operation_id: "operation-0".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Running,
            payload: payload.clone(),
            created_at: second.created_at,
            updated_at: second.updated_at,
            finished_at: None,
        };
        repository
            .update_operation(&first)
            .await
            .expect_err("missing operation should fail");
        repository
            .operations
            .lock()
            .expect("operation lock should succeed")
            .insert(first.operation_id.clone(), first.clone());

        let operations = repository
            .list_running()
            .await
            .expect("operations should load");

        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.operation_id.as_str())
                .collect::<Vec<_>>(),
            vec!["operation-0", second.operation_id.as_str()]
        );
    }

    #[tokio::test]
    async fn latest_for_workspace_prefers_created_at_updated_at_then_operation_id_descending() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        let older = LifecycleOperation {
            operation_id: "operation-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Completed,
            payload: payload.clone(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        };
        let latest = LifecycleOperation {
            operation_id: "operation-2".to_string(),
            workspace_id: "workspace-1".to_string(),
            state: LifecycleOperationState::Failed,
            payload,
            created_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(1),
            updated_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: Some(OffsetDateTime::UNIX_EPOCH),
        };
        {
            let mut operations = repository
                .operations
                .lock()
                .expect("operation lock should succeed");
            operations.insert(older.operation_id.clone(), older);
            operations.insert(latest.operation_id.clone(), latest.clone());
        }

        let operation = repository
            .latest_for_workspace(&"workspace-1".to_string())
            .await
            .expect("operation should load")
            .expect("operation should exist");

        assert_eq!(operation.operation_id, latest.operation_id);
    }

    #[tokio::test]
    async fn delete_for_workspace_removes_only_matching_operations() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        repository
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("first operation should be created");
        let remaining = repository
            .create_operation(&"workspace-2".to_string(), &payload)
            .await
            .expect("second operation should be created");

        repository
            .delete_for_workspace(&"workspace-1".to_string())
            .await
            .expect("workspace operations should delete");

        assert_eq!(
            repository
                .latest_for_workspace(&"workspace-1".to_string())
                .await
                .expect("latest should load"),
            None
        );
        assert_eq!(
            repository
                .latest_for_workspace(&"workspace-2".to_string())
                .await
                .expect("latest should load")
                .expect("remaining operation should exist")
                .operation_id,
            remaining.operation_id
        );
    }
}
