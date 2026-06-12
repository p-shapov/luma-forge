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
            LifecycleOperationState, WorkspaceId,
        },
        provisioned_remote::{
            GpuCloudProviderId, ProvisionedRemoteProvisionerStatus,
            RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits,
            RemoteGpuPlacementOption, RemotePlacementOptions, RemotePlacementPlan,
        },
        workspace::{Workspace, WorkspaceCatalog},
    },
    lifecycle_journal::{LifecycleJournalError, LifecycleJournalRepository},
    shared::{AppFuture, BackgroundTask, BackgroundTaskSpawner, NoopEventSink},
    workflow_catalog::WorkflowCatalogService,
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

use super::{
    errors::ProvisionedRemoteError,
    lifecycle::{
        self,
        runner::{
            BackgroundProvisionedRemoteLifecycleRunner, ProvisionedRemoteLifecycleRunner,
            ProvisionedRemoteLifecycleRunnerContext,
        },
    },
    provider::{
        CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
        GetProvisionerStatusParams, ProvisionedRemoteEndpointProvider,
        ProvisionedRemotePlacementOptionsProvider, ProvisionedRemoteProvider,
        ProvisionedRemoteProvisionerProvider, ProvisionedRemoteVolumeProvider,
        StartProvisionerParams, TerminateProvisionerParams,
    },
    registry::ProvisionedRemoteProviderRegistry,
    service::{CreateProvisionedRemoteWorkspaceRequest, ProvisionedRemoteService},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestBackgroundTaskSpawner;

impl BackgroundTaskSpawner for TestBackgroundTaskSpawner {
    fn spawn(&self, task: BackgroundTask) {
        tokio::spawn(task);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManualLifecycleRunner;

impl<W, L> ProvisionedRemoteLifecycleRunner<W, L> for ManualLifecycleRunner
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    fn spawn_provision(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            context.lifecycle_operation_registry.complete(&operation_id);
        }
    }

    fn spawn_cleanup(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            context.lifecycle_operation_registry.complete(&operation_id);
        }
    }

    fn spawn_delete(
        &self,
        context: ProvisionedRemoteLifecycleRunnerContext<W, L>,
        operation_id: LifecycleOperationId,
    ) {
        if context
            .lifecycle_operation_registry
            .try_register(&operation_id)
        {
            context.lifecycle_operation_registry.complete(&operation_id);
        }
    }
}

pub(crate) trait ManualLifecycleRunnerExt {
    fn run_provision_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn run_cleanup_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn run_delete_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
}

impl<W, L> ManualLifecycleRunnerExt for ProvisionedRemoteService<W, L>
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    fn run_provision_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::provision::run_once(
                &operation_id,
                &context.workspace_repository,
                &context.lifecycle_journal,
                &context.workflow_catalog,
                &context.provider_registry,
                &context.event_sink,
                Duration::ZERO,
            )
            .await;
            context.lifecycle_operation_registry.complete(&operation_id);
            result
        })
    }

    fn run_cleanup_once_for_test<'a>(
        &'a self,
        operation_id: &'a str,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::cleanup::run_once(
                &operation_id,
                &context.workspace_repository,
                &context.lifecycle_journal,
                &context.provider_registry,
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
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let operation_id = operation_id.to_string();
            let context = self.lifecycle_runner_context();
            let result = lifecycle::delete::run_once(
                &operation_id,
                &context.workspace_repository,
                &context.lifecycle_journal,
                &context.provider_registry,
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
pub(crate) struct ProviderState {
    pub(crate) calls: Vec<&'static str>,
    pub(crate) provisioner_image_refs: Vec<String>,
    pub(crate) endpoint_image_refs: Vec<String>,
    pub(crate) placement_options_result:
        Option<Result<RemotePlacementOptions, ProvisionedRemoteError>>,
    pub(crate) provisioner_status_results: Vec<ProvisionedRemoteProvisionerStatus>,
    pub(crate) create_volume_error: Option<ProvisionedRemoteError>,
    pub(crate) start_provisioner_error: Option<ProvisionedRemoteError>,
    pub(crate) terminate_provisioner_error: Option<ProvisionedRemoteError>,
    pub(crate) get_provisioner_status_error: Option<ProvisionedRemoteError>,
    pub(crate) create_endpoint_error: Option<ProvisionedRemoteError>,
    pub(crate) delete_endpoint_error: Option<ProvisionedRemoteError>,
    pub(crate) delete_volume_error: Option<ProvisionedRemoteError>,
}

#[derive(Clone, Default)]
pub(crate) struct WorkspaceRepositoryState {
    pub(crate) delete_workspace_error: Option<WorkspaceCatalogError>,
}

pub(crate) fn placement_options() -> RemotePlacementOptions {
    RemotePlacementOptions {
        max_persistent_storage_volume_size_bytes: Some(10),
        datacenters: vec![RemoteDatacenterPlacementOption {
            id: "dc".to_string(),
            name: "Datacenter".to_string(),
            gpu_options: vec![RemoteGpuPlacementOption {
                id: "gpu".to_string(),
                name: "GPU".to_string(),
                vram_bytes: 24,
                availability_score: 90,
            }],
        }],
    }
}

struct FakeProvider {
    state: Arc<Mutex<ProviderState>>,
}

impl FakeProvider {
    fn new(state: Arc<Mutex<ProviderState>>) -> Self {
        Self { state }
    }
}

impl ProvisionedRemotePlacementOptionsProvider for FakeProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("get_provider_placement_options");

            state
                .placement_options_result
                .clone()
                .unwrap_or_else(|| Ok(placement_options()))
        })
    }
}

impl ProvisionedRemoteVolumeProvider for FakeProvider {
    fn create_volume<'a>(
        &'a self,
        _params: CreateVolumeParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("create_volume");
            if let Some(error) = state.create_volume_error.clone() {
                return Err(error);
            }

            Ok("volume".to_string())
        })
    }

    fn delete_volume<'a>(
        &'a self,
        _params: DeleteVolumeParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("delete_volume");
            if let Some(error) = state.delete_volume_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }
}

impl ProvisionedRemoteProvisionerProvider for FakeProvider {
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("start_provisioner");
            state
                .provisioner_image_refs
                .push(params.provisioner_image_ref);
            if let Some(error) = state.start_provisioner_error.clone() {
                return Err(error);
            }

            Ok("provisioner".to_string())
        })
    }

    fn terminate_provisioner<'a>(
        &'a self,
        _params: TerminateProvisionerParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("terminate_provisioner");
            if let Some(error) = state.terminate_provisioner_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        _params: GetProvisionerStatusParams,
    ) -> AppFuture<'a, Result<ProvisionedRemoteProvisionerStatus, ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("get_provisioner_status");
            if let Some(error) = state.get_provisioner_status_error.clone() {
                return Err(error);
            }

            if state.provisioner_status_results.is_empty() {
                Ok(ProvisionedRemoteProvisionerStatus::Pending)
            } else {
                Ok(state.provisioner_status_results.remove(0))
            }
        })
    }
}

impl ProvisionedRemoteEndpointProvider for FakeProvider {
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("create_endpoint");
            state.endpoint_image_refs.push(params.endpoint_image_ref);
            if let Some(error) = state.create_endpoint_error.clone() {
                return Err(error);
            }

            Ok("endpoint".to_string())
        })
    }

    fn delete_endpoint<'a>(
        &'a self,
        _params: DeleteEndpointParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("state lock should succeed");
            state.calls.push("delete_endpoint");
            if let Some(error) = state.delete_endpoint_error.clone() {
                return Err(error);
            }

            Ok(())
        })
    }
}

impl ProvisionedRemoteProvider for FakeProvider {
    fn provider_id(&self) -> GpuCloudProviderId {
        GpuCloudProviderId::Runpod
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

pub(crate) fn draft_create_request(workspace_id: &str) -> CreateProvisionedRemoteWorkspaceRequest {
    CreateProvisionedRemoteWorkspaceRequest {
        workspace_id: workspace_id.to_string(),
        workflow_preset_id: "comfyui-hidream-o1-dev".to_string(),
        remote_placement: placement_plan(),
    }
}

pub(crate) fn service_with_state(
    state: Arc<Mutex<ProviderState>>,
) -> ProvisionedRemoteService<InMemoryWorkspaceRepository, InMemoryLifecycleJournalRepository> {
    ProvisionedRemoteService::new(
        InMemoryWorkspaceRepository::default(),
        InMemoryLifecycleJournalRepository::default(),
        WorkflowCatalogService::new(),
        ProvisionedRemoteProviderRegistry::new(vec![Box::new(FakeProvider::new(state))]),
        Arc::new(NoopEventSink::new()),
        Arc::new(TestBackgroundTaskSpawner),
        Arc::new(BackgroundProvisionedRemoteLifecycleRunner),
    )
}

pub(crate) fn service_without_lifecycle_spawning(
    state: Arc<Mutex<ProviderState>>,
) -> ProvisionedRemoteService<InMemoryWorkspaceRepository, InMemoryLifecycleJournalRepository> {
    ProvisionedRemoteService::new(
        InMemoryWorkspaceRepository::default(),
        InMemoryLifecycleJournalRepository::default(),
        WorkflowCatalogService::new(),
        ProvisionedRemoteProviderRegistry::new(vec![Box::new(FakeProvider::new(state))]),
        Arc::new(NoopEventSink::new()),
        Arc::new(TestBackgroundTaskSpawner),
        Arc::new(ManualLifecycleRunner),
    )
}

pub(crate) fn service_with_state_and_workspace_repository(
    provider_state: Arc<Mutex<ProviderState>>,
    workspace_repository: InMemoryWorkspaceRepository,
) -> ProvisionedRemoteService<InMemoryWorkspaceRepository, InMemoryLifecycleJournalRepository> {
    ProvisionedRemoteService::new(
        workspace_repository,
        InMemoryLifecycleJournalRepository::default(),
        WorkflowCatalogService::new(),
        ProvisionedRemoteProviderRegistry::new(vec![Box::new(FakeProvider::new(provider_state))]),
        Arc::new(NoopEventSink::new()),
        Arc::new(TestBackgroundTaskSpawner),
        Arc::new(BackgroundProvisionedRemoteLifecycleRunner),
    )
}

fn placement_plan() -> RemotePlacementPlan {
    RemotePlacementPlan {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        datacenter_id: "dc".to_string(),
        gpu_id: "gpu".to_string(),
        volume_size_bytes: 1,
        keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
            default_seconds: 60,
            min_seconds: 30,
            max_seconds: 120,
        }),
    }
}

pub(crate) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };

    fn raw_waker() -> RawWaker {
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        fn wake(_: *const ()) {}
        fn wake_by_ref(_: *const ()) {}
        fn drop(_: *const ()) {}

        RawWaker::new(
            std::ptr::null(),
            &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
        )
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);

    loop {
        match Pin::new(&mut future).poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::provisioned_remote::ProvisionedRemoteLifecycleOperationPayload;

    fn provision_payload() -> LifecycleOperationPayload {
        LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision {
                step: None,
                error: None,
            },
        )
    }

    #[test]
    fn update_operation_returns_not_found_for_missing_operation() {
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

        let error = block_on(repository.update_operation(&operation))
            .expect_err("missing operation should not be upserted");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[test]
    fn mark_state_returns_not_found_for_terminal_operation() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        let operation = block_on(repository.create_operation(&"workspace-1".to_string(), &payload))
            .expect("operation should be created");
        block_on(repository.mark_state(
            &operation.operation_id,
            LifecycleOperationState::Completed,
            &payload,
        ))
        .expect("operation should complete");

        let error = block_on(repository.mark_state(
            &operation.operation_id,
            LifecycleOperationState::Stale,
            &payload,
        ))
        .expect_err("terminal operation should not be marked again");

        assert_eq!(error, LifecycleJournalError::OperationNotFound);
    }

    #[test]
    fn list_running_returns_created_at_then_operation_id_order() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        let second = block_on(repository.create_operation(&"workspace-2".to_string(), &payload))
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
        block_on(repository.update_operation(&first)).expect_err("missing operation should fail");
        repository
            .operations
            .lock()
            .expect("operation lock should succeed")
            .insert(first.operation_id.clone(), first.clone());

        let operations = block_on(repository.list_running()).expect("operations should load");

        assert_eq!(
            operations
                .iter()
                .map(|operation| operation.operation_id.as_str())
                .collect::<Vec<_>>(),
            vec!["operation-0", second.operation_id.as_str()]
        );
    }

    #[test]
    fn latest_for_workspace_prefers_created_at_updated_at_then_operation_id_descending() {
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
        let mut operations = repository
            .operations
            .lock()
            .expect("operation lock should succeed");
        operations.insert(older.operation_id.clone(), older);
        operations.insert(latest.operation_id.clone(), latest.clone());
        drop(operations);

        let operation = block_on(repository.latest_for_workspace(&"workspace-1".to_string()))
            .expect("operation should load")
            .expect("operation should exist");

        assert_eq!(operation.operation_id, latest.operation_id);
    }

    #[test]
    fn delete_for_workspace_removes_only_matching_operations() {
        let repository = InMemoryLifecycleJournalRepository::default();
        let payload = provision_payload();
        block_on(repository.create_operation(&"workspace-1".to_string(), &payload))
            .expect("first operation should be created");
        let remaining = block_on(repository.create_operation(&"workspace-2".to_string(), &payload))
            .expect("second operation should be created");

        block_on(repository.delete_for_workspace(&"workspace-1".to_string()))
            .expect("workspace operations should delete");

        assert_eq!(
            block_on(repository.latest_for_workspace(&"workspace-1".to_string()))
                .expect("latest should load"),
            None
        );
        assert_eq!(
            block_on(repository.latest_for_workspace(&"workspace-2".to_string()))
                .expect("latest should load")
                .expect("remaining operation should exist")
                .operation_id,
            remaining.operation_id
        );
    }
}
