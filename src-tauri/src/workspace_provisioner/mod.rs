mod gateway;

pub(crate) use gateway::{
    progress_from_worker_status, ProvisionerWorkerError, ProvisionerWorkerGateway,
    ProvisionerWorkerHttpGateway, ProvisionerWorkerJobStatus, ProvisionerWorkerStartRequest,
};

use crate::{
    domain::workspace::{
        provisioning_state::fail_workspace, ProviderResourceStatus, Workspace,
        WorkspaceProvisioningPhase, WorkspaceProvisioningProgress, WorkspaceProvisioningStatus,
    },
    secrets::{SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_provisioning::{
        failure,
        helpers::{result, WorkspaceProvisioningResult},
        WorkspaceProvisioningError,
    },
};

pub(crate) type WorkspaceProvisionerSyncResult =
    Result<Option<WorkspaceProvisioningResult>, WorkspaceProvisioningError>;

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
        S: SecretStore,
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

        let token = match context.secrets.read_provisioner_worker_token(&workspace.id) {
            Ok(Some(token)) => token,
            Ok(None) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_missing(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Err(SecretStoreError::InvalidStoredProvisionerWorkerToken) => {
                fail_workspace(
                    workspace,
                    failure::worker_token_invalid(WorkspaceProvisioningPhase::PreparingEnvironment),
                );
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
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
                        &ProvisionerWorkerStartRequest {
                            job_id: workspace.id.clone(),
                            workflow_preset: workspace
                                .placement_plan
                                .selected_workflow_preset()
                                .clone(),
                            resolved_runtime_image: workspace.resolved_runtime_image.clone(),
                        },
                    )
                    .await
                {
                    Ok(status) => status,
                    Err(error) => {
                        return handle_worker_error(&context, workspace.clone(), error.into()).await
                    }
                }
            }
            Ok(status) if status.status == ProvisionerWorkerJobStatus::Succeeded => {
                workspace.environment_prepared_at = Some(now_rfc3339()?);
                let workspace = context.update_workspace(workspace).await?;
                return Ok(Some(result(workspace)));
            }
            Ok(status) => status,
            Err(error) => {
                return handle_worker_error(&context, workspace.clone(), error.into()).await
            }
        };

        Ok(Some(WorkspaceProvisioningResult {
            workspace: workspace.clone(),
            progress: progress_from_worker_status(&worker_status),
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
    error: WorkspaceProvisioningError,
) -> WorkspaceProvisionerSyncResult
where
    W: WorkspaceCatalogRepository,
{
    if error == WorkspaceProvisioningError::ProvisionerWorkerUnavailable {
        return Ok(Some(WorkspaceProvisioningResult {
            workspace,
            progress: worker_readiness_progress(),
        }));
    }

    if let Some(failure) =
        failure::worker_failure(WorkspaceProvisioningPhase::PreparingEnvironment, &error)
    {
        fail_workspace(&mut workspace, failure);
        let workspace = context.update_workspace(&workspace).await?;
        Ok(Some(result(workspace)))
    } else {
        Err(error)
    }
}

fn worker_readiness_progress() -> WorkspaceProvisioningProgress {
    WorkspaceProvisioningProgress {
        status: WorkspaceProvisioningStatus::Running,
        phase: WorkspaceProvisioningPhase::PreparingEnvironment,
        percent: None,
        failure: None,
    }
}

fn now_rfc3339() -> Result<String, WorkspaceProvisioningError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| WorkspaceProvisioningError::ProviderResponseInvalid)
}

fn catalog_error(
    _error: crate::workspace_setup::error::WorkspaceSetupError,
) -> WorkspaceProvisioningError {
    WorkspaceProvisioningError::WorkspaceCatalogUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            placement::PlacementPlan,
            provider_setup::{GpuCloudProviderId, ProviderApiKey},
            runtime::ResolvedRuntimeImageSnapshot,
            workflow::{RuntimeContractReference, WorkflowExecutionType, WorkflowPreset},
            workspace::{
                ProvisioningPodSnapshot, WorkspaceCatalog, WorkspaceLifecycleState,
                WorkspaceProvisioningFailureCode,
            },
        },
        secrets::ProvisionerWorkerBearerToken,
        workspace_setup::error::WorkspaceSetupError,
    };
    use gateway::ProvisionerWorkerStatus;
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

    impl SecretStore for FakeSecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            Ok(false)
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            Ok(None)
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            _api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            Ok(())
        }

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
    }

    #[derive(Debug, Default)]
    struct FakeProvisionerWorkerGateway {
        status_calls: Mutex<Vec<String>>,
        start_calls: Mutex<Vec<String>>,
    }

    impl FakeProvisionerWorkerGateway {
        fn status_calls(&self) -> Vec<String> {
            self.status_calls.lock().expect("fake status calls").clone()
        }

        fn start_calls(&self) -> Vec<String> {
            self.start_calls.lock().expect("fake start calls").clone()
        }
    }

    impl ProvisionerWorkerGateway for FakeProvisionerWorkerGateway {
        fn start<'a>(
            &'a self,
            provisioner_status_url: &'a str,
            _token: &'a ProvisionerWorkerBearerToken,
            _request: &'a ProvisionerWorkerStartRequest,
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
            Box::pin(async { Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None }) })
        }

        fn status<'a>(
            &'a self,
            provisioner_status_url: &'a str,
            _token: &'a ProvisionerWorkerBearerToken,
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
            Box::pin(async { Err(ProvisionerWorkerError::InvalidPayload { diagnostic: None }) })
        }
    }

    fn running_workspace() -> Workspace {
        let preset = WorkflowPreset {
            id: "preset-1".to_string(),
            version: "1.0.0".to_string(),
            name: "Preset".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: 1,
            runtime_contract: RuntimeContractReference {
                id: "runtime".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: Vec::new(),
            required_custom_nodes: Vec::new(),
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
            provisioner_image_ref: "provisioner:latest".to_string(),
            endpoint_image_ref: "endpoint:latest".to_string(),
        };
        let mut workspace = Workspace::new_draft(
            GpuCloudProviderId::Runpod,
            "workspace-1".to_string(),
            "Workspace".to_string(),
            placement_plan,
            runtime,
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

    async fn sync_with_token_result(
        token_result: Result<Option<String>, SecretStoreError>,
    ) -> (
        WorkspaceProvisioningResult,
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

    #[tokio::test]
    async fn sync_environment_fails_workspace_when_worker_token_is_missing() {
        let (result, secrets, catalog, workers) = sync_with_token_result(Ok(None)).await;

        assert_eq!(secrets.read_worker_token_calls(), vec!["workspace-1"]);
        assert!(workers.status_calls().is_empty());
        assert!(workers.start_calls().is_empty());
        assert_eq!(catalog.updates().len(), 1);
        assert_eq!(
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        assert_eq!(
            result
                .progress
                .failure
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
            result.workspace.lifecycle_state,
            WorkspaceLifecycleState::Failed
        );
        assert_eq!(
            result
                .progress
                .failure
                .expect("failure should be present")
                .code,
            WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenInvalid
        );
    }
}
