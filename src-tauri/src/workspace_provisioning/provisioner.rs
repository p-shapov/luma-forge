use crate::{
    domain::workspace::{ProviderResourceStatus, Workspace, WorkspaceProvisioningPhase},
    secrets::{AsyncProvisionerTokenStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use super::{
    failure::{self, fail_workspace},
    gateway::{
        ProvisionerWorkerError, ProvisionerWorkerGateway, ProvisionerWorkerJobStatus,
        ProvisionerWorkerStartRequest, ProvisionerWorkerStatus,
    },
    helpers::catalog_error,
    WorkspaceProvisioningError,
};

pub(crate) type WorkspaceProvisionerSyncResult =
    Result<Option<WorkspaceProvisionerSyncOutcome>, WorkspaceProvisioningError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceProvisionerSyncOutcome {
    WorkspaceUpdated(Workspace),
    WorkerReadinessLag {
        workspace: Workspace,
    },
    WorkerStatus {
        workspace: Workspace,
        status: ProvisionerWorkerStatus,
    },
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkspaceProvisionerService;

impl WorkspaceProvisionerService {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) async fn sync_environment<S, W, R>(
        &self,
        context: WorkspaceProvisionerContext<'_, S, W, R>,
        workspace: &mut Workspace,
    ) -> WorkspaceProvisionerSyncResult
    where
        S: AsyncProvisionerTokenStore,
        W: WorkspaceCatalogRepository,
        R: ProvisionerWorkerGateway,
    {
        if workspace.environment_prepared_at.is_some() {
            return Ok(None);
        }

        let Some(active_pod) = workspace.active_provisioning_pod_snapshot.clone() else {
            return Ok(None);
        };

        if active_pod.provider_resource_status != ProviderResourceStatus::Running {
            return Ok(None);
        }

        let token = match context
            .secrets
            .read_provisioner_worker_token(&workspace.id)
            .await
        {
            Ok(Some(token)) => token,
            Ok(None) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_missing(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(
                    workspace,
                )));
            }
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_invalid(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(
                    workspace,
                )));
            }
            Err(error) => return Err(WorkspaceProvisioningError::from(error)),
        };

        let worker_status = match context
            .workers
            .status(&active_pod.provisioner_status_url, &token)
            .await
        {
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Idle => {
                match context
                    .workers
                    .start(
                        &active_pod.provisioner_status_url,
                        &token,
                        &ProvisionerWorkerStartRequest::from_workflow_preset(
                            workspace.id.clone(),
                            workspace.placement_plan.selected_workflow_preset(),
                        ),
                    )
                    .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        return handle_worker_error(&context, workspace.clone(), error).await
                    }
                }
            }
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
                workspace.environment_prepared_at = Some(now_rfc3339()?);
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(
                    workspace,
                )));
            }
            Ok(status) => status,
            Err(error) => return handle_worker_error(&context, workspace.clone(), error).await,
        };

        Ok(Some(WorkspaceProvisionerSyncOutcome::WorkerStatus {
            workspace: workspace.clone(),
            status: worker_status,
        }))
    }
}

pub(crate) struct WorkspaceProvisionerContext<'a, S, W, R> {
    secrets: &'a S,
    workspace_catalog: &'a W,
    workers: &'a R,
}

impl<'a, S, W, R> WorkspaceProvisionerContext<'a, S, W, R> {
    pub(crate) fn new(secrets: &'a S, workspace_catalog: &'a W, workers: &'a R) -> Self {
        Self {
            secrets,
            workspace_catalog,
            workers,
        }
    }
}

impl<S, W, R> WorkspaceProvisionerContext<'_, S, W, R>
where
    W: WorkspaceCatalogRepository,
{
    async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceProvisioningError> {
        self.workspace_catalog
            .update_workspace(workspace)
            .await
            .map_err(catalog_error)
    }
}

async fn handle_worker_error<S, W, R>(
    context: &WorkspaceProvisionerContext<'_, S, W, R>,
    mut workspace: Workspace,
    error: ProvisionerWorkerError,
) -> WorkspaceProvisionerSyncResult
where
    W: WorkspaceCatalogRepository,
{
    if error == ProvisionerWorkerError::Unreachable {
        return Ok(Some(WorkspaceProvisionerSyncOutcome::WorkerReadinessLag {
            workspace,
        }));
    }

    if let Some(failure) = failure::provisioner_worker_failure(
        WorkspaceProvisioningPhase::PreparingEnvironment,
        &error,
    ) {
        fail_workspace(&mut workspace, failure);
        let workspace = context.update_workspace(&workspace).await?;
        Ok(Some(WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(
            workspace,
        )))
    } else {
        Err(WorkspaceProvisioningError::from(error))
    }
}

fn now_rfc3339() -> Result<String, WorkspaceProvisioningError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| WorkspaceProvisioningError::ProviderResponseInvalid)
}

#[cfg(test)]
mod tests {
    use super::super::gateway::{ProvisionerWorkerPhase, ProvisionerWorkerStatus};
    use super::*;
    use crate::{
        domain::{
            placement::PlacementPlan,
            provider_setup::GpuCloudProviderId,
            provisioner::ResolvedProvisionerImageSnapshot,
            runtime::ResolvedRuntimeImageSnapshot,
            workflow::{
                ModelAsset, ModelAssetSource, ProvisionerContractReference,
                RuntimeContractReference, WorkflowExecutionType, WorkflowPreset,
            },
            workspace::{
                ProvisioningPodSnapshot, WorkspaceCatalog, WorkspaceLifecycleState,
                WorkspaceProvisioningFailureCode, WorkspaceProvisioningFailureSource,
            },
        },
        secrets::{ProvisionerTokenStore, ProvisionerWorkerBearerToken},
        workspace_setup::error::WorkspaceSetupError,
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone)]
    struct FakeSecretStore {
        token_result: Arc<Mutex<Result<Option<String>, SecretStoreError>>>,
        read_worker_token_calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakeSecretStore {
        fn new(token_result: Result<Option<String>, SecretStoreError>) -> Self {
            Self {
                token_result: Arc::new(Mutex::new(token_result)),
                read_worker_token_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn read_worker_token_calls(&self) -> Vec<String> {
            self.read_worker_token_calls
                .lock()
                .expect("fake secret calls")
                .clone()
        }
    }

    impl ProvisionerTokenStore for FakeSecretStore {
        fn write_provisioner_worker_token(
            &self,
            _workspace_id: &str,
            _token: &ProvisionerWorkerBearerToken,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }

        fn read_provisioner_worker_token(
            &self,
            workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            self.read_worker_token_calls
                .lock()
                .expect("fake secret calls")
                .push(workspace_id.to_string());

            self.token_result
                .lock()
                .expect("fake token result")
                .clone()
                .map(|token| {
                    token
                        .map(ProvisionerWorkerBearerToken::new)
                        .transpose()
                        .expect("test token should be valid")
                })
        }

        fn delete_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FakeWorkspaceCatalog {
        updates: Mutex<Vec<Workspace>>,
    }

    impl FakeWorkspaceCatalog {
        fn updates(&self) -> Vec<Workspace> {
            self.updates.lock().expect("fake catalog updates").clone()
        }
    }

    impl WorkspaceCatalogRepository for FakeWorkspaceCatalog {
        fn list_workspaces<'a>(
            &'a self,
        ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            _id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn insert_workspace<'a>(
            &'a self,
            _workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>>
        {
            Box::pin(async move {
                self.updates
                    .lock()
                    .expect("fake catalog updates")
                    .push(workspace.clone());
                Ok(workspace.clone())
            })
        }

        fn delete_workspace<'a>(
            &'a self,
            _id: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), WorkspaceSetupError>> + Send + 'a>> {
            Box::pin(async { Err(WorkspaceSetupError::WorkspaceCatalogUnavailable) })
        }
    }

    #[derive(Debug, Default)]
    struct FakeProvisionerWorkerGateway {
        status_calls: Mutex<Vec<String>>,
        status_tokens: Mutex<Vec<String>>,
        status_result: Mutex<Option<Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>>,
        start_calls: Mutex<Vec<String>>,
        start_tokens: Mutex<Vec<String>>,
        start_requests: Mutex<Vec<ProvisionerWorkerStartRequest>>,
        start_result: Mutex<Option<Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>>,
    }

    impl FakeProvisionerWorkerGateway {
        fn with_status_result(
            result: Result<ProvisionerWorkerStatus, ProvisionerWorkerError>,
        ) -> Self {
            Self {
                status_result: Mutex::new(Some(result)),
                ..Self::default()
            }
        }

        fn with_status_and_start_results(
            status_result: Result<ProvisionerWorkerStatus, ProvisionerWorkerError>,
            start_result: Result<ProvisionerWorkerStatus, ProvisionerWorkerError>,
        ) -> Self {
            Self {
                status_result: Mutex::new(Some(status_result)),
                start_result: Mutex::new(Some(start_result)),
                ..Self::default()
            }
        }

        fn status_calls(&self) -> Vec<String> {
            self.status_calls.lock().expect("fake status calls").clone()
        }

        fn status_tokens(&self) -> Vec<String> {
            self.status_tokens
                .lock()
                .expect("fake status tokens")
                .clone()
        }

        fn start_calls(&self) -> Vec<String> {
            self.start_calls.lock().expect("fake start calls").clone()
        }

        fn start_tokens(&self) -> Vec<String> {
            self.start_tokens.lock().expect("fake start tokens").clone()
        }

        fn start_requests(&self) -> Vec<ProvisionerWorkerStartRequest> {
            self.start_requests
                .lock()
                .expect("fake start requests")
                .clone()
        }
    }

    impl ProvisionerWorkerGateway for FakeProvisionerWorkerGateway {
        fn start<'a>(
            &'a self,
            provisioner_status_url: &'a str,
            token: &'a ProvisionerWorkerBearerToken,
            request: &'a ProvisionerWorkerStartRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                    + Send
                    + 'a,
            >,
        > {
            self.start_calls
                .lock()
                .expect("fake start calls")
                .push(provisioner_status_url.to_string());
            self.start_tokens
                .lock()
                .expect("fake start tokens")
                .push(token.expose_secret().to_string());
            self.start_requests
                .lock()
                .expect("fake start requests")
                .push(request.clone());
            let result = self
                .start_result
                .lock()
                .expect("fake start result")
                .clone()
                .unwrap_or(Err(ProvisionerWorkerError::InvalidPayload));
            Box::pin(async move { result })
        }

        fn status<'a>(
            &'a self,
            provisioner_status_url: &'a str,
            token: &'a ProvisionerWorkerBearerToken,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                    + Send
                    + 'a,
            >,
        > {
            self.status_calls
                .lock()
                .expect("fake status calls")
                .push(provisioner_status_url.to_string());
            self.status_tokens
                .lock()
                .expect("fake status tokens")
                .push(token.expose_secret().to_string());
            let result = self
                .status_result
                .lock()
                .expect("fake status result")
                .clone()
                .unwrap_or(Err(ProvisionerWorkerError::InvalidPayload));
            Box::pin(async move { result })
        }
    }

    fn running_workspace() -> Workspace {
        let preset = WorkflowPreset {
            id: "preset-1".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 1,
            requires_hugging_face_api_key: false,
            runtime_contract: RuntimeContractReference {
                id: "runtime".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: ProvisionerContractReference {
                id: "provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![ModelAsset {
                id: "model-1".to_string(),
                name: "Model One".to_string(),
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/model".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                install_comfyui_relative_path: "models/checkpoints/model.safetensors".to_string(),
            }],
        };
        let placement_plan = PlacementPlan::Runpod {
            selected_datacenter_id: "dc-1".to_string(),
            selected_gpu_id: "gpu-1".to_string(),
            persistent_storage_volume_size_bytes: 1,
            endpoint_keep_alive_seconds: 5,
            selected_workflow_preset: preset,
        };
        let runtime = ResolvedRuntimeImageSnapshot {
            contract_id: "runtime".to_string(),
            contract_version: "1.0.0".to_string(),
            endpoint_image_ref: "endpoint:latest".to_string(),
        };
        let provisioner = ResolvedProvisionerImageSnapshot {
            contract_id: "provisioner".to_string(),
            contract_version: "1.0.0".to_string(),
            provisioner_worker_image_ref: "provisioner:latest".to_string(),
        };
        let mut workspace = Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-1".to_string(),
            "Workspace".to_string(),
            placement_plan,
            runtime,
            provisioner,
        )
        .expect("workspace should be valid");
        workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
        workspace.active_provisioning_pod_snapshot = Some(ProvisioningPodSnapshot {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_resource_id: "pod-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Running,
            provisioner_status_url: "https://worker.example/status".to_string(),
        });
        workspace
    }

    fn worker_status(
        status: ProvisionerWorkerJobStatus,
        phase: ProvisionerWorkerPhase,
        progress_percent: Option<u8>,
    ) -> ProvisionerWorkerStatus {
        ProvisionerWorkerStatus {
            status,
            phase,
            progress_percent,
        }
    }

    async fn sync_workspace(
        workspace: &mut Workspace,
        workers: &FakeProvisionerWorkerGateway,
    ) -> WorkspaceProvisionerSyncResult {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();

        service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, workers),
                workspace,
            )
            .await
    }

    async fn sync_with_token_result(
        token_result: Result<Option<String>, SecretStoreError>,
    ) -> (
        WorkspaceProvisionerSyncOutcome,
        FakeSecretStore,
        FakeWorkspaceCatalog,
        FakeProvisionerWorkerGateway,
    ) {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(token_result);
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::default();
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("sync should not return infrastructure error")
            .expect("sync should return failed workspace result");

        (result, secrets, catalog, workers)
    }

    fn outcome_workspace(outcome: &WorkspaceProvisionerSyncOutcome) -> &Workspace {
        match outcome {
            WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(workspace)
            | WorkspaceProvisionerSyncOutcome::WorkerReadinessLag { workspace }
            | WorkspaceProvisionerSyncOutcome::WorkerStatus { workspace, .. } => workspace,
        }
    }

    #[tokio::test]
    async fn sync_environment_returns_none_when_environment_is_already_prepared() {
        let workers = FakeProvisionerWorkerGateway::default();
        let mut workspace = running_workspace();
        workspace.environment_prepared_at = Some("2026-05-18T00:00:00Z".to_string());

        let result = sync_workspace(&mut workspace, &workers)
            .await
            .expect("sync should not fail");

        assert!(result.is_none());
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
    }

    #[tokio::test]
    async fn sync_environment_returns_none_without_active_provisioning_pod() {
        let workers = FakeProvisionerWorkerGateway::default();
        let mut workspace = running_workspace();
        workspace.active_provisioning_pod_snapshot = None;

        let result = sync_workspace(&mut workspace, &workers)
            .await
            .expect("sync should not fail");

        assert!(result.is_none());
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
    }

    #[tokio::test]
    async fn sync_environment_returns_none_until_active_pod_is_running() {
        let workers = FakeProvisionerWorkerGateway::default();
        let mut workspace = running_workspace();
        workspace
            .active_provisioning_pod_snapshot
            .as_mut()
            .expect("active pod")
            .provider_resource_status = ProviderResourceStatus::Creating;

        let result = sync_workspace(&mut workspace, &workers)
            .await
            .expect("sync should not fail");

        assert!(result.is_none());
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
    }

    #[tokio::test]
    async fn sync_environment_reports_readiness_progress_when_worker_is_unavailable() {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::with_status_result(Err(
            ProvisionerWorkerError::Unreachable,
        ));
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("worker readiness lag should not be an infrastructure error")
            .expect("readiness progress should be returned");

        assert_eq!(secrets.read_worker_token_calls(), vec!["workspace-1"]);
        assert_eq!(
            workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        assert_eq!(workers.status_tokens(), vec!["worker-token"]);
        assert!(workers.start_calls().is_empty());
        assert!(catalog.updates().is_empty());
        assert!(matches!(
            result,
            WorkspaceProvisionerSyncOutcome::WorkerReadinessLag { .. }
        ));
        assert_eq!(
            outcome_workspace(&result).lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
    }

    #[tokio::test]
    async fn sync_environment_starts_idle_worker_with_workspace_context() {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::with_status_and_start_results(
            Ok(worker_status(
                ProvisionerWorkerJobStatus::Idle,
                ProvisionerWorkerPhase::Idle,
                None,
            )),
            Ok(worker_status(
                ProvisionerWorkerJobStatus::Running,
                ProvisionerWorkerPhase::PreparingWorkspace,
                Some(37),
            )),
        );
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("sync should not fail")
            .expect("worker progress should be returned");

        assert_eq!(
            workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        assert_eq!(workers.start_calls(), vec!["https://worker.example/status"]);
        assert_eq!(workers.start_tokens(), vec!["worker-token"]);
        assert!(catalog.updates().is_empty());
        assert_eq!(
            result,
            WorkspaceProvisionerSyncOutcome::WorkerStatus {
                workspace: workspace.clone(),
                status: worker_status(
                    ProvisionerWorkerJobStatus::Running,
                    ProvisionerWorkerPhase::PreparingWorkspace,
                    Some(37),
                ),
            }
        );

        let request = workers
            .start_requests()
            .pop()
            .expect("start request should be captured");
        assert_eq!(request.job_id, "workspace-1");
        let request_json = serde_json::to_value(&request).expect("start request should serialize");
        assert_eq!(
            request_json,
            serde_json::json!({
                "job_id": "workspace-1",
                    "workflow_preset": {
                        "requires_hugging_face_api_key": false,
                        "required_model_assets": [
                        {
                            "id": "model-1",
                            "name": "Model One",
                            "download_source": {
                                "source_type": "huggingface",
                                "repository_id": "owner/model",
                                "file_path": "model.safetensors",
                                "revision": "main",
                            },
                            "install_comfyui_relative_path": "models/checkpoints/model.safetensors",
                        },
                    ],
                },
            })
        );
    }

    #[tokio::test]
    async fn sync_environment_returns_worker_progress_without_persisting_workspace() {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::with_status_result(Ok(worker_status(
            ProvisionerWorkerJobStatus::Running,
            ProvisionerWorkerPhase::DownloadingAssets,
            Some(42),
        )));
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("sync should not fail")
            .expect("worker progress should be returned");

        assert_eq!(
            workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        assert!(workers.start_calls().is_empty());
        assert!(catalog.updates().is_empty());
        assert_eq!(
            result,
            WorkspaceProvisionerSyncOutcome::WorkerStatus {
                workspace,
                status: worker_status(
                    ProvisionerWorkerJobStatus::Running,
                    ProvisionerWorkerPhase::DownloadingAssets,
                    Some(42),
                ),
            }
        );
    }

    #[tokio::test]
    async fn sync_environment_persists_prepared_timestamp_when_worker_succeeds() {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::with_status_result(Ok(worker_status(
            ProvisionerWorkerJobStatus::Succeeded,
            ProvisionerWorkerPhase::Completed,
            Some(100),
        )));
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("sync should not fail")
            .expect("prepared workspace should be returned");

        let updates = catalog.updates();
        assert_eq!(updates.len(), 1);
        assert!(updates[0].environment_prepared_at.is_some());
        assert!(updates[0].active_provisioning_pod_snapshot.is_some());
        assert_eq!(
            result,
            WorkspaceProvisionerSyncOutcome::WorkspaceUpdated(updates[0].clone())
        );
    }

    #[tokio::test]
    async fn sync_environment_fails_workspace_for_unauthorized_worker() {
        let (result, _, catalog, workers) =
            sync_with_worker_error(ProvisionerWorkerError::Unauthorized).await;

        assert_eq!(
            workers.status_calls(),
            vec!["https://worker.example/status"]
        );
        assert_eq!(catalog.updates().len(), 1);
        assert_eq!(
            outcome_workspace(&result).lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        let failure = outcome_workspace(&result)
            .last_provisioning_failure
            .clone()
            .expect("failure should be present");
        assert_eq!(
            failure.code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerUnauthorized
        );
        assert_eq!(
            failure.source,
            WorkspaceProvisioningFailureSource::ProvisionerWorker
        );
    }

    #[tokio::test]
    async fn sync_environment_fails_workspace_for_invalid_worker_response() {
        let (result, _, catalog, _) =
            sync_with_worker_error(ProvisionerWorkerError::InvalidPayload).await;

        assert_eq!(catalog.updates().len(), 1);
        let failure = outcome_workspace(&result)
            .last_provisioning_failure
            .clone()
            .expect("failure should be present");
        assert_eq!(
            failure.code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid
        );
    }

    #[tokio::test]
    async fn sync_environment_fails_workspace_for_worker_terminal_failure() {
        let (result, _, catalog, _) =
            sync_with_worker_error(ProvisionerWorkerError::AssetDownloadFailed).await;

        assert_eq!(catalog.updates().len(), 1);
        let failure = outcome_workspace(&result)
            .last_provisioning_failure
            .clone()
            .expect("failure should be present");
        assert_eq!(
            failure.code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerAssetDownloadFailed
        );
    }

    #[tokio::test]
    async fn sync_environment_propagates_worker_conflict_without_persisting_failure() {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers =
            FakeProvisionerWorkerGateway::with_status_result(Err(ProvisionerWorkerError::Conflict));
        let mut workspace = running_workspace();

        let error = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect_err("conflict should remain a command-level error");

        assert_eq!(error, WorkspaceProvisioningError::ProvisionerWorkerConflict);
        assert!(catalog.updates().is_empty());
        assert_eq!(
            workspace.lifecycle_state,
            WorkspaceLifecycleState::Provisioning
        );
    }

    async fn sync_with_worker_error(
        error: ProvisionerWorkerError,
    ) -> (
        WorkspaceProvisionerSyncOutcome,
        FakeSecretStore,
        FakeWorkspaceCatalog,
        FakeProvisionerWorkerGateway,
    ) {
        let service = WorkspaceProvisionerService::new();
        let secrets = FakeSecretStore::new(Ok(Some("worker-token".to_string())));
        let catalog = FakeWorkspaceCatalog::default();
        let workers = FakeProvisionerWorkerGateway::with_status_result(Err(error));
        let mut workspace = running_workspace();

        let result = service
            .sync_environment(
                WorkspaceProvisionerContext::new(&secrets, &catalog, &workers),
                &mut workspace,
            )
            .await
            .expect("sync should not return infrastructure error")
            .expect("sync should return failed workspace result");

        (result, secrets, catalog, workers)
    }

    #[tokio::test]
    async fn sync_environment_fails_workspace_when_worker_token_is_missing() {
        let (result, secrets, catalog, workers) = sync_with_token_result(Ok(None)).await;

        assert_eq!(secrets.read_worker_token_calls(), vec!["workspace-1"]);
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
        assert_eq!(catalog.updates().len(), 1);
        assert_eq!(
            outcome_workspace(&result).lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        assert_eq!(
            outcome_workspace(&result)
                .last_provisioning_failure
                .clone()
                .expect("failure should be present")
                .code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing
        );
    }

    #[tokio::test]
    async fn sync_environment_fails_workspace_when_worker_token_is_invalid() {
        let (result, secrets, catalog, workers) =
            sync_with_token_result(Err(SecretStoreError::InvalidStoredProvisionerWorkerToken))
                .await;

        assert_eq!(secrets.read_worker_token_calls(), vec!["workspace-1"]);
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
        assert_eq!(catalog.updates().len(), 1);
        assert_eq!(
            outcome_workspace(&result).lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        assert_eq!(
            outcome_workspace(&result)
                .last_provisioning_failure
                .clone()
                .expect("failure should be present")
                .code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid
        );
    }
}
