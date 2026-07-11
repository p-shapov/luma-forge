#![allow(dead_code)]

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

use secrecy::SecretString;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    catalog::{
        CatalogRef, RunpodContractRequirements, RunpodRuntimeDefinition, RuntimeContract,
        RuntimeContractRequirements, RuntimePreset, WorkflowDefinition, WorkflowSummary,
    },
    events::{ApplicationEvent, ApplicationEventSink},
    lifecycle::{
        ports::{LifecycleOperationRepository, LifecycleOperationRepositoryError},
        progress::runpod::{RunpodCleanupStep, RunpodProvisionStep},
        LifecycleOperation, LifecycleOperationState,
    },
    runtimes::ports::{RuntimeTransitionRepository, RuntimeTransitionRepositoryError},
    secrets::{SecretKind, SecretStore, SecretStoreError},
    workspace::{
        ports::{
            WorkflowCatalog, WorkflowCatalogError, WorkspaceRepository, WorkspaceRepositoryError,
        },
        RuntimeKind, Workspace,
    },
};

use super::{
    CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodRuntime, RunpodRuntimeCatalog,
    RunpodRuntimeCatalogError, RunpodRuntimeConfig, RunpodRuntimeProvider,
    RunpodRuntimeProviderError, RunpodRuntimeRepository, RunpodRuntimeRepositoryError,
    RunpodRuntimeResources, RunpodRuntimeService, RunpodRuntimeServiceDependencies,
    RunpodRuntimeState, StartProvisionerPod,
};

pub(super) struct ProvisionFakes {
    pub provider: Arc<FakeRunpodRuntimeProvider>,
    pub repository: Arc<FakeRunpodRuntimeRepository>,
    workspaces: Arc<FakeWorkspaceRepository>,
    workflows: Arc<FakeWorkflowCatalog>,
    runtime_catalog: Arc<FakeRunpodRuntimeCatalog>,
    lifecycle: Arc<FakeLifecycleOperationRepository>,
    secrets: Arc<FakeSecretStore>,
    pub events: Arc<RecordingApplicationEventSink>,
    workspace_rows: Arc<Mutex<Vec<Workspace>>>,
}

pub(super) type CleanupFakes = ProvisionFakes;
pub(super) type RecoveryFakes = ProvisionFakes;

impl ProvisionFakes {
    pub fn ready() -> Self {
        Self::new(workspace(None), None, Vec::new())
    }

    pub fn ready_without_runpod_credential() -> Self {
        let fakes = Self::ready();
        fakes.secrets.runpod.store(false, Ordering::Relaxed);
        fakes
    }

    pub fn ready_runtime() -> Self {
        let mut runtime = runtime(RunpodRuntimeState::Ready);
        runtime.resources = RunpodRuntimeResources {
            network_volume_id: Some("volume-1".into()),
            provisioner_pod_id: Some("pod-1".into()),
            template_id: Some("template-1".into()),
            endpoint_id: Some("endpoint-1".into()),
        };
        Self::new(
            workspace(Some(RuntimeKind::Runpod)),
            Some(runtime),
            Vec::new(),
        )
    }

    pub fn ready_runtime_without_runpod_credential() -> Self {
        let fakes = Self::ready_runtime();
        fakes.secrets.runpod.store(false, Ordering::Relaxed);
        fakes
    }

    pub fn failed_partial_runtime() -> Self {
        let mut runtime = runtime(RunpodRuntimeState::Failed);
        runtime.resources = RunpodRuntimeResources {
            network_volume_id: Some("volume-1".into()),
            provisioner_pod_id: None,
            template_id: None,
            endpoint_id: None,
        };
        Self::new(
            workspace(Some(RuntimeKind::Runpod)),
            Some(runtime),
            Vec::new(),
        )
    }

    pub fn without_runtime() -> Self {
        Self::new(workspace(None), None, Vec::new())
    }

    pub fn with_running_provision_and_cleanup() -> Self {
        let now = OffsetDateTime::UNIX_EPOCH;
        let fakes = Self::new(
            workspace(Some(RuntimeKind::Runpod)),
            Some(runtime(RunpodRuntimeState::Provisioning)),
            vec![
                LifecycleOperation::runpod_provision(
                    Uuid::from_u128(1),
                    "workspace-1",
                    Uuid::from_u128(2),
                    RunpodProvisionStep::CreateEndpoint,
                    now,
                ),
                LifecycleOperation::runpod_cleanup(
                    Uuid::from_u128(3),
                    "workspace-2",
                    Uuid::from_u128(4),
                    RunpodCleanupStep::DeleteEndpoint,
                    now,
                ),
            ],
        );
        let mut cleanup_runtime = runtime(RunpodRuntimeState::CleaningUp);
        cleanup_runtime.workspace_id = "workspace-2".into();
        fakes
            .repository
            .runtimes
            .lock()
            .unwrap()
            .push(cleanup_runtime);
        fakes
    }

    pub fn service(&self) -> RunpodRuntimeService {
        RunpodRuntimeService::new(RunpodRuntimeServiceDependencies {
            workspaces: self.workspaces.clone(),
            workflows: self.workflows.clone(),
            runtimes: self.repository.clone(),
            runtime_catalog: self.runtime_catalog.clone(),
            lifecycle: self.lifecycle.clone(),
            secrets: self.secrets.clone(),
            provider: self.provider.clone(),
            events: self.events.clone(),
        })
    }

    pub fn workspace_snapshot(&self) -> Workspace {
        self.workspace_rows
            .lock()
            .unwrap()
            .iter()
            .find(|workspace| workspace.id == "workspace-1")
            .cloned()
            .expect("workspace fixture should exist")
    }

    fn new(
        workspace: Workspace,
        runtime: Option<RunpodRuntime>,
        operations: Vec<LifecycleOperation>,
    ) -> Self {
        let workspace_rows = Arc::new(Mutex::new(vec![workspace]));
        Self {
            provider: Arc::new(FakeRunpodRuntimeProvider::default()),
            repository: Arc::new(FakeRunpodRuntimeRepository::new(
                runtime,
                workspace_rows.clone(),
            )),
            workspaces: Arc::new(FakeWorkspaceRepository(workspace_rows.clone())),
            workflows: Arc::new(FakeWorkflowCatalog(workflow())),
            runtime_catalog: Arc::new(FakeRunpodRuntimeCatalog(runtime_definition())),
            lifecycle: Arc::new(FakeLifecycleOperationRepository(Mutex::new(operations))),
            secrets: Arc::new(FakeSecretStore {
                runpod: AtomicBool::new(true),
                hugging_face: true,
            }),
            events: Arc::new(RecordingApplicationEventSink::default()),
            workspace_rows,
        }
    }
}

#[derive(Default)]
pub(super) struct RecordingApplicationEventSink {
    events: Mutex<Vec<ApplicationEvent>>,
    changed: tokio::sync::Notify,
}

impl ApplicationEventSink for RecordingApplicationEventSink {
    fn emit(&self, event: ApplicationEvent) {
        self.events.lock().unwrap().push(event);
        self.changed.notify_waiters();
    }
}

impl RecordingApplicationEventSink {
    pub fn events(&self) -> Vec<ApplicationEvent> {
        self.events.lock().unwrap().clone()
    }

    pub async fn wait_for_terminal_operation(&self, id: Uuid) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let found = self.events.lock().unwrap().iter().any(|event| {
                matches!(
                    event,
                    ApplicationEvent::LifecycleOperationChanged(operation)
                        if operation.id == id
                            && operation.state != LifecycleOperationState::Running
                )
            });
            if found {
                return;
            }
            changed.await;
        }
    }

    pub fn runtime_changed_count(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ApplicationEvent::RuntimeChanged(_)))
            .count()
    }

    pub fn runtime_deleted_count(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ApplicationEvent::RuntimeDeleted { .. }))
            .count()
    }

    pub fn lifecycle_event_count(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ApplicationEvent::LifecycleOperationChanged(_)))
            .count()
    }

    pub fn workspace_event_count(&self) -> usize {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| matches!(event, ApplicationEvent::WorkspaceChanged(_)))
            .count()
    }
}

pub fn provision_command() -> super::ProvisionRunpodRuntime {
    super::ProvisionRunpodRuntime {
        workspace_id: "workspace-1".into(),
        datacenter_id: "dc-1".into(),
        gpu_id: "gpu-1".into(),
        volume_size_gb: 19,
    }
}

fn workspace(attached_runtime: Option<RuntimeKind>) -> Workspace {
    Workspace {
        id: "workspace-1".into(),
        workflow: CatalogRef::new("workflow-1", "1"),
        created_at: OffsetDateTime::UNIX_EPOCH,
        attached_runtime,
    }
}

fn workflow() -> WorkflowDefinition {
    WorkflowDefinition {
        summary: WorkflowSummary {
            id: "workflow-1".into(),
            revision: "1".into(),
            name: "Workflow".into(),
            description: String::new(),
            required_volume_size_gb: 19,
            requires_hugging_face_api_key: true,
        },
        runtime_preset_ref: CatalogRef::new("runpod-preset", "1"),
        contract_requirements: vec![RuntimeContractRequirements::Runpod(
            RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
            },
        )],
        model_assets: serde_json::json!([{"id": "model-1"}]),
        execution_contract: serde_json::json!({}),
        workflow_graph: serde_json::json!({}),
    }
}

fn runtime_definition() -> RunpodRuntimeDefinition {
    RunpodRuntimeDefinition {
        runtime_preset: RuntimePreset(serde_json::json!({})),
        provisioner_contract: RuntimeContract {
            image_ref: "provisioner:1".into(),
        },
        endpoint_contract: RuntimeContract {
            image_ref: "endpoint:1".into(),
        },
    }
}

fn runtime(state: RunpodRuntimeState) -> RunpodRuntime {
    RunpodRuntime {
        workspace_id: "workspace-1".into(),
        state,
        config: RunpodRuntimeConfig {
            datacenter_id: "dc-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb: 19,
        },
        resources: RunpodRuntimeResources::default(),
    }
}

struct FakeWorkspaceRepository(Arc<Mutex<Vec<Workspace>>>);

#[async_trait::async_trait]
impl WorkspaceRepository for FakeWorkspaceRepository {
    async fn create(&self, workspace: Workspace) -> Result<Workspace, WorkspaceRepositoryError> {
        self.0.lock().unwrap().push(workspace.clone());
        Ok(workspace)
    }

    async fn get(&self, id: &str) -> Result<Option<Workspace>, WorkspaceRepositoryError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.id == id)
            .cloned())
    }

    async fn list(&self) -> Result<Vec<Workspace>, WorkspaceRepositoryError> {
        Ok(self.0.lock().unwrap().clone())
    }

    async fn delete(&self, id: &str) -> Result<bool, WorkspaceRepositoryError> {
        let mut workspaces = self.0.lock().unwrap();
        let before = workspaces.len();
        workspaces.retain(|item| item.id != id);
        Ok(workspaces.len() != before)
    }
}

struct FakeWorkflowCatalog(WorkflowDefinition);

#[async_trait::async_trait]
impl WorkflowCatalog for FakeWorkflowCatalog {
    async fn list_summaries(&self) -> Result<Vec<WorkflowSummary>, WorkflowCatalogError> {
        Ok(vec![self.0.summary.clone()])
    }

    async fn get(
        &self,
        id: &str,
        revision: &str,
    ) -> Result<Option<WorkflowDefinition>, WorkflowCatalogError> {
        Ok(
            (self.0.summary.id == id && self.0.summary.revision == revision)
                .then(|| self.0.clone()),
        )
    }
}

struct FakeRunpodRuntimeCatalog(RunpodRuntimeDefinition);

#[async_trait::async_trait]
impl RunpodRuntimeCatalog for FakeRunpodRuntimeCatalog {
    async fn resolve(
        &self,
        _: &CatalogRef,
        _: &RunpodContractRequirements,
    ) -> Result<RunpodRuntimeDefinition, RunpodRuntimeCatalogError> {
        Ok(self.0.clone())
    }
}

struct FakeLifecycleOperationRepository(Mutex<Vec<LifecycleOperation>>);

#[async_trait::async_trait]
impl LifecycleOperationRepository for FakeLifecycleOperationRepository {
    async fn recent(
        &self,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn recent_for_workspace(
        &self,
        workspace_id: &str,
        limit: u64,
    ) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|operation| operation.workspace_id == workspace_id)
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn running(&self) -> Result<Vec<LifecycleOperation>, LifecycleOperationRepositoryError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|operation| operation.state == LifecycleOperationState::Running)
            .cloned()
            .collect())
    }

    async fn has_running(
        &self,
        workspace_id: &str,
    ) -> Result<bool, LifecycleOperationRepositoryError> {
        Ok(self.0.lock().unwrap().iter().any(|operation| {
            operation.workspace_id == workspace_id
                && operation.state == LifecycleOperationState::Running
        }))
    }
}

struct FakeSecretStore {
    runpod: AtomicBool,
    hugging_face: bool,
}

#[async_trait::async_trait]
impl SecretStore for FakeSecretStore {
    async fn exists(&self, kind: SecretKind) -> Result<bool, SecretStoreError> {
        Ok(match kind {
            SecretKind::RunpodApiKey => self.runpod.load(Ordering::Relaxed),
            SecretKind::HuggingFaceApiKey => self.hugging_face,
        })
    }

    async fn get(&self, kind: SecretKind) -> Result<Option<SecretString>, SecretStoreError> {
        Ok(self
            .exists(kind)
            .await?
            .then(|| SecretString::from("secret")))
    }

    async fn insert(&self, _: SecretKind, _: SecretString) -> Result<(), SecretStoreError> {
        Ok(())
    }

    async fn delete(&self, _: SecretKind) -> Result<(), SecretStoreError> {
        Ok(())
    }
}

pub(super) struct FakeRunpodRuntimeRepository {
    runtimes: Mutex<Vec<RunpodRuntime>>,
    snapshots: Mutex<Vec<(RunpodRuntime, LifecycleOperation)>>,
    workspace_rows: Arc<Mutex<Vec<Workspace>>>,
    save_count: AtomicUsize,
    fail_on_save: AtomicUsize,
    failed_write_attempted: AtomicBool,
    changed: tokio::sync::Notify,
}

impl FakeRunpodRuntimeRepository {
    fn new(runtime: Option<RunpodRuntime>, workspace_rows: Arc<Mutex<Vec<Workspace>>>) -> Self {
        Self {
            runtimes: Mutex::new(runtime.into_iter().collect()),
            snapshots: Mutex::new(Vec::new()),
            workspace_rows,
            save_count: AtomicUsize::new(0),
            fail_on_save: AtomicUsize::new(usize::MAX),
            failed_write_attempted: AtomicBool::new(false),
            changed: tokio::sync::Notify::new(),
        }
    }

    pub fn fail_transition_after_initial_commit(&self) {
        self.fail_on_save.store(2, Ordering::SeqCst);
    }

    pub async fn wait_for_failed_transition(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.failed_write_attempted.load(Ordering::SeqCst) {
                return;
            }
            changed.await;
        }
    }

    pub fn running_steps(&self) -> Vec<RunpodProvisionStep> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, operation)| operation.state == LifecycleOperationState::Running)
            .filter_map(|(_, operation)| operation.progress.provision_step())
            .collect()
    }

    pub fn running_cleanup_steps(&self) -> Vec<RunpodCleanupStep> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, operation)| operation.state == LifecycleOperationState::Running)
            .filter_map(|(_, operation)| operation.progress.cleanup_step())
            .collect()
    }

    pub fn runtime_was_removed(&self) -> bool {
        self.runtimes.lock().unwrap().is_empty()
    }

    pub fn runtime_state(&self, workspace_id: &str) -> Option<RunpodRuntimeState> {
        self.runtimes
            .lock()
            .unwrap()
            .iter()
            .find(|runtime| runtime.workspace_id == workspace_id)
            .map(|runtime| runtime.state)
    }

    pub fn saved_states(&self) -> Vec<(RunpodRuntimeState, LifecycleOperationState)> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .map(|(runtime, operation)| (runtime.state, operation.state))
            .collect()
    }

    pub fn saved_trace_ids(&self) -> Vec<Uuid> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .map(|(_, operation)| operation.trace_id)
            .collect()
    }

    pub fn last_operation_state(&self) -> LifecycleOperationState {
        self.snapshots.lock().unwrap().last().unwrap().1.state
    }

    pub fn last_snapshot(&self) -> (RunpodRuntime, LifecycleOperation) {
        self.snapshots.lock().unwrap().last().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl RunpodRuntimeRepository for FakeRunpodRuntimeRepository {
    async fn get(
        &self,
        workspace_id: &str,
    ) -> Result<Option<RunpodRuntime>, RunpodRuntimeRepositoryError> {
        Ok(self
            .runtimes
            .lock()
            .unwrap()
            .iter()
            .find(|runtime| runtime.workspace_id == workspace_id)
            .cloned())
    }
}

#[async_trait::async_trait]
impl RuntimeTransitionRepository<RunpodRuntime> for FakeRunpodRuntimeRepository {
    async fn save_transition(
        &self,
        runtime: &RunpodRuntime,
        operation: &LifecycleOperation,
    ) -> Result<(), RuntimeTransitionRepositoryError> {
        let save_number = self.save_count.fetch_add(1, Ordering::SeqCst) + 1;
        if save_number == self.fail_on_save.load(Ordering::SeqCst) {
            self.failed_write_attempted.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
            return Err(RuntimeTransitionRepositoryError::Unavailable);
        }
        let mut runtimes = self.runtimes.lock().unwrap();
        if runtime.state == RunpodRuntimeState::CleaningUp
            && operation.state == LifecycleOperationState::Succeeded
        {
            runtimes.retain(|item| item.workspace_id != runtime.workspace_id);
            if let Some(workspace) = self
                .workspace_rows
                .lock()
                .unwrap()
                .iter_mut()
                .find(|workspace| workspace.id == runtime.workspace_id)
            {
                workspace.attached_runtime = None;
            }
        } else if let Some(saved) = runtimes
            .iter_mut()
            .find(|item| item.workspace_id == runtime.workspace_id)
        {
            *saved = runtime.clone();
        } else {
            runtimes.push(runtime.clone());
            if let Some(workspace) = self
                .workspace_rows
                .lock()
                .unwrap()
                .iter_mut()
                .find(|workspace| workspace.id == runtime.workspace_id)
            {
                workspace.attached_runtime = Some(RuntimeKind::Runpod);
            }
        }
        self.snapshots
            .lock()
            .unwrap()
            .push((runtime.clone(), operation.clone()));
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct FakeRunpodRuntimeProvider {
    calls: Mutex<Vec<&'static str>>,
    fail_once: Mutex<Option<&'static str>>,
    block_first_call: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl FakeRunpodRuntimeProvider {
    pub fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }

    pub fn fail_once(&self, method: &'static str) {
        *self.fail_once.lock().unwrap() = Some(method);
    }

    pub fn block_first_call(&self) {
        self.block_first_call.store(true, Ordering::SeqCst);
    }

    pub async fn wait_until_first_call(&self) {
        self.entered.notified().await;
    }

    pub fn release_first_call(&self) {
        self.release.notify_one();
    }

    async fn call(&self, method: &'static str) -> Result<(), RunpodRuntimeProviderError> {
        self.calls.lock().unwrap().push(method);
        if self.block_first_call.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        let mut fail_once = self.fail_once.lock().unwrap();
        if *fail_once == Some(method) {
            *fail_once = None;
            Err(RunpodRuntimeProviderError::Unavailable)
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl RunpodRuntimeProvider for FakeRunpodRuntimeProvider {
    async fn create_network_volume(
        &self,
        _: &SecretString,
        _: CreateNetworkVolume,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.call("create_network_volume").await?;
        Ok("volume-1".into())
    }

    async fn start_provisioner_pod(
        &self,
        _: &SecretString,
        _: StartProvisionerPod,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.call("start_provisioner_pod").await?;
        Ok("pod-1".into())
    }

    async fn wait_for_provisioner(
        &self,
        _: &SecretString,
        _: &str,
        _: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        self.call("wait_for_provisioner").await
    }

    async fn terminate_provisioner_pod(
        &self,
        _: &SecretString,
        _: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        self.call("terminate_provisioner_pod").await
    }

    async fn create_template(
        &self,
        _: &SecretString,
        _: CreateTemplate,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.call("create_template").await?;
        Ok("template-1".into())
    }

    async fn create_endpoint(
        &self,
        _: &SecretString,
        _: CreateEndpoint,
    ) -> Result<String, RunpodRuntimeProviderError> {
        self.call("create_endpoint").await?;
        Ok("endpoint-1".into())
    }

    async fn delete_endpoint(
        &self,
        _: &SecretString,
        _: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        self.call("delete_endpoint").await
    }

    async fn delete_template(
        &self,
        _: &SecretString,
        _: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        self.call("delete_template").await
    }

    async fn delete_network_volume(
        &self,
        _: &SecretString,
        _: &str,
    ) -> Result<(), RunpodRuntimeProviderError> {
        self.call("delete_network_volume").await
    }
}
