pub mod api;
pub mod config;
pub mod mapping;
pub mod provisioner;

use std::sync::Arc;

use crate::{
    domain::{
        placement::RemotePlacementOptions,
        provider::{GpuCloudProviderId, ProviderApiError},
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteVolumeSnapshot,
        },
    },
    remote_workspace::{
        errors::RemoteWorkspaceError,
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
            RemoteProvisionerProvider, RemoteVolumeProvider, RemoteWorkspaceProvider,
            StartProvisionerParams, TerminateProvisionerParams,
        },
    },
    secrets_storage::{
        ApiKeyIdentityProvider, SecretStore, SecretsStorageError, SecretsStorageService,
    },
    shared::AppFuture,
};

use self::{
    api::{
        CreateEndpointRequest, CreateNetworkVolumeRequest, CreateProvisionerPodRequest,
        HttpRunpodApi, RunpodApi,
    },
    config::{
        DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS, NETWORK_VOLUME_MAX_SIZE_BYTES, PROVISIONER_PORT,
        RUNPOD_GRAPHQL_URL, RUNPOD_REST_BASE_URL,
    },
    provisioner::{ProvisionerWorkerApi, ProvisionerWorkerClient},
};

pub struct RunpodRemoteWorkspaceProvider<RS, RI, HS, HI> {
    api: Arc<dyn RunpodApi>,
    provisioner_worker: Arc<dyn ProvisionerWorkerApi>,
    runpod_secrets: Arc<SecretsStorageService<RS, RI>>,
    hugging_face_secrets: SecretsStorageService<HS, HI>,
}

impl<RS, RI, HS, HI> RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore + 'static,
    RI: ApiKeyIdentityProvider + 'static,
{
    pub fn new(
        runpod_secrets: SecretsStorageService<RS, RI>,
        hugging_face_secrets: SecretsStorageService<HS, HI>,
    ) -> Self {
        let http = reqwest::Client::new();
        let runpod_secrets = Arc::new(runpod_secrets);
        Self::with_shared_clients(
            Arc::new(HttpRunpodApi::new(
                http.clone(),
                RUNPOD_REST_BASE_URL.to_string(),
                RUNPOD_GRAPHQL_URL.to_string(),
                Arc::clone(&runpod_secrets),
            )),
            Arc::new(ProvisionerWorkerClient::new(http)),
            runpod_secrets,
            hugging_face_secrets,
        )
    }

    #[cfg(test)]
    fn with_clients(
        api: Arc<dyn RunpodApi>,
        provisioner_worker: Arc<dyn ProvisionerWorkerApi>,
        runpod_secrets: SecretsStorageService<RS, RI>,
        hugging_face_secrets: SecretsStorageService<HS, HI>,
    ) -> Self {
        let runpod_secrets = Arc::new(runpod_secrets);
        Self::with_shared_clients(
            api,
            provisioner_worker,
            runpod_secrets,
            hugging_face_secrets,
        )
    }

    fn with_shared_clients(
        api: Arc<dyn RunpodApi>,
        provisioner_worker: Arc<dyn ProvisionerWorkerApi>,
        runpod_secrets: Arc<SecretsStorageService<RS, RI>>,
        hugging_face_secrets: SecretsStorageService<HS, HI>,
    ) -> Self {
        Self {
            api,
            provisioner_worker,
            runpod_secrets,
            hugging_face_secrets,
        }
    }
}

impl<RS, RI, HS, HI> RemotePlacementOptionsProvider
    for RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: Send + Sync,
    HI: Send + Sync,
{
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
        Box::pin(async move {
            let mut options = self.api.placement_options().await?;
            options.max_persistent_storage_volume_size_bytes = Some(NETWORK_VOLUME_MAX_SIZE_BYTES);
            Ok(options)
        })
    }
}

impl<RS, RI, HS, HI> RemoteVolumeProvider for RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: Send + Sync,
    HI: Send + Sync,
{
    fn create_volume<'a>(
        &'a self,
        params: CreateVolumeParams,
    ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>> {
        Box::pin(async move {
            self.api
                .create_network_volume(CreateNetworkVolumeRequest {
                    datacenter_id: params.datacenter_id,
                    name: mapping::workspace_resource_name(&params.workspace_id, "volume"),
                    size_gb: mapping::bytes_to_runpod_volume_gb(params.size_bytes),
                })
                .await
        })
    }

    fn delete_volume<'a>(
        &'a self,
        params: DeleteVolumeParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async move { self.api.delete_network_volume(&params.volume_id).await })
    }
}

impl<RS, RI, HS, HI> RemoteProvisionerProvider for RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerSnapshot, RemoteWorkspaceError>> {
        Box::pin(async move {
            let bearer_token = self
                .runpod_secrets
                .hmac_sha256_hex(&params.workspace_id)
                .await
                .map_err(map_secret_error)?;
            let hugging_face_api_key = if params.requires_hugging_face_api_key {
                Some(
                    self.hugging_face_secrets
                        .retrieve()
                        .await
                        .map_err(map_secret_error)?
                        .expose_secret()
                        .to_string(),
                )
            } else {
                None
            };
            let pod = self
                .api
                .create_provisioner_pod(CreateProvisionerPodRequest {
                    datacenter_id: params.datacenter_id,
                    image_ref: params.provisioner_image_ref,
                    network_volume_id: params.volume_id,
                    mount_path: params.mount_path,
                    bearer_token,
                    hugging_face_api_key,
                })
                .await?;

            Ok(RemoteProvisionerSnapshot {
                status_url: provisioner_status_url(&pod.id),
                id: pod.id,
            })
        })
    }

    fn terminate_provisioner<'a>(
        &'a self,
        params: TerminateProvisionerParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async move {
            self.api
                .delete_provisioner_pod(&params.provisioner_id)
                .await
        })
    }

    fn get_provisioner_status<'a>(
        &'a self,
        params: GetProvisionerStatusParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
        Box::pin(async move {
            let bearer_token = self
                .runpod_secrets
                .hmac_sha256_hex(&params.workspace_id)
                .await
                .map_err(map_secret_error)?;

            self.provisioner_worker
                .get_status(&params.status_url, &bearer_token)
                .await
                .map_err(RemoteWorkspaceError::ProvisionerWorker)
        })
    }
}

impl<RS, RI, HS, HI> RemoteEndpointProvider for RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: Send + Sync,
    HI: Send + Sync,
{
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
        Box::pin(async move {
            let endpoint = self
                .api
                .create_endpoint(CreateEndpointRequest {
                    datacenter_id: params.datacenter_id,
                    gpu_id: params.gpu_id,
                    image_ref: params.endpoint_image_ref,
                    network_volume_id: params.volume_id,
                    mount_path: params.mount_path,
                    keep_alive_limits: params
                        .keep_alive_limits
                        .unwrap_or(DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS),
                })
                .await?;

            Ok(RemoteEndpointSnapshot {
                id: endpoint.id,
                url: endpoint.url,
            })
        })
    }

    fn delete_endpoint<'a>(
        &'a self,
        params: DeleteEndpointParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
        Box::pin(async move {
            self.api
                .delete_endpoint_and_template(&params.endpoint_id)
                .await
        })
    }
}

impl<RS, RI, HS, HI> RemoteWorkspaceProvider for RunpodRemoteWorkspaceProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    fn provider_id(&self) -> GpuCloudProviderId {
        GpuCloudProviderId::Runpod
    }
}

fn provisioner_status_url(pod_id: &str) -> String {
    format!("https://{pod_id}-{PROVISIONER_PORT}.proxy.runpod.net/status")
}

fn map_secret_error(_error: SecretsStorageError) -> RemoteWorkspaceError {
    ProviderApiError::RequestFailed {
        message: "required provider secret is unavailable".to_string(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use crate::{
        domain::{
            placement::{
                RemoteDatacenterPlacementOption, RemoteEndpointKeepAliveLimits,
                RemoteGpuPlacementOption,
            },
            secrets::ApiKeyIdentity,
            workspace::RemoteProvisioningError,
        },
        secrets_storage::{ApiSecret, SecretKey, SecretStore},
    };

    use self::api::{RunpodEndpoint, RunpodId};
    use super::*;

    #[derive(Default)]
    struct ApiState {
        create_volume_requests: Vec<CreateNetworkVolumeRequest>,
        provisioner_pod_requests: Vec<CreateProvisionerPodRequest>,
        endpoint_requests: Vec<CreateEndpointRequest>,
        deleted_endpoints: Vec<String>,
    }

    struct FakeApi {
        state: Arc<Mutex<ApiState>>,
    }

    impl RunpodApi for FakeApi {
        fn placement_options<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
            Box::pin(async {
                Ok(RemotePlacementOptions {
                    max_persistent_storage_volume_size_bytes: None,
                    datacenters: vec![RemoteDatacenterPlacementOption {
                        id: "dc".to_string(),
                        name: "Datacenter".to_string(),
                        gpu_options: vec![RemoteGpuPlacementOption {
                            id: "gpu".to_string(),
                            name: "GPU".to_string(),
                            vram_bytes: 24_000_000_000,
                            availability_score: 100,
                        }],
                    }],
                })
            })
        }

        fn create_network_volume<'a>(
            &'a self,
            request: CreateNetworkVolumeRequest,
        ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .create_volume_requests
                    .push(request);
                Ok(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_network_volume<'a>(
            &'a self,
            _volume_id: &'a str,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async { Ok(()) })
        }

        fn create_provisioner_pod<'a>(
            &'a self,
            request: CreateProvisionerPodRequest,
        ) -> AppFuture<'a, Result<RunpodId, RemoteWorkspaceError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .provisioner_pod_requests
                    .push(request);
                Ok(RunpodId {
                    id: "pod".to_string(),
                })
            })
        }

        fn delete_provisioner_pod<'a>(
            &'a self,
            _pod_id: &'a str,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async { Ok(()) })
        }

        fn create_endpoint<'a>(
            &'a self,
            request: CreateEndpointRequest,
        ) -> AppFuture<'a, Result<RunpodEndpoint, RemoteWorkspaceError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .endpoint_requests
                    .push(request);
                Ok(RunpodEndpoint {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                    template_id: "template".to_string(),
                })
            })
        }

        fn delete_endpoint_and_template<'a>(
            &'a self,
            endpoint_id: &'a str,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .deleted_endpoints
                    .push(endpoint_id.to_string());
                Ok(())
            })
        }
    }

    #[derive(Default)]
    struct WorkerState {
        calls: Vec<(String, String)>,
        result: Option<Result<RemoteProvisionerStatus, RemoteProvisioningError>>,
    }

    struct FakeWorker {
        state: Arc<Mutex<WorkerState>>,
    }

    impl ProvisionerWorkerApi for FakeWorker {
        fn get_status<'a>(
            &'a self,
            status_url: &'a str,
            bearer_token: &'a str,
        ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteProvisioningError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("worker state");
                state
                    .calls
                    .push((status_url.to_string(), bearer_token.to_string()));
                state
                    .result
                    .clone()
                    .unwrap_or(Ok(RemoteProvisionerStatus::Running))
            })
        }
    }

    #[derive(Clone, Default)]
    struct FakeStore {
        secrets: Arc<Mutex<HashMap<SecretKey, ApiSecret>>>,
    }

    impl FakeStore {
        fn insert(&self, key: SecretKey, value: &str) {
            self.secrets
                .lock()
                .expect("secrets")
                .insert(key, ApiSecret::new(value.to_string()).expect("secret"));
        }
    }

    impl SecretStore for FakeStore {
        fn has<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<bool, SecretsStorageError>> {
            Box::pin(async move { Ok(self.secrets.lock().expect("secrets").contains_key(&key)) })
        }

        fn write<'a>(
            &'a self,
            key: SecretKey,
            secret: ApiSecret,
        ) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async move {
                self.secrets.lock().expect("secrets").insert(key, secret);
                Ok(())
            })
        }

        fn delete<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async move {
                self.secrets.lock().expect("secrets").remove(&key);
                Ok(())
            })
        }

        fn read<'a>(
            &'a self,
            key: SecretKey,
        ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>> {
            Box::pin(async move { Ok(self.secrets.lock().expect("secrets").get(&key).cloned()) })
        }
    }

    #[derive(Clone)]
    struct FakeIdentityProvider;

    impl ApiKeyIdentityProvider for FakeIdentityProvider {
        fn identity<'a>(
            &'a self,
            _secret: &'a ApiSecret,
        ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
            Box::pin(async {
                Ok(ApiKeyIdentity {
                    email: None,
                    username: None,
                    key_display_name: None,
                })
            })
        }
    }

    fn provider(
        api_state: Arc<Mutex<ApiState>>,
        worker_state: Arc<Mutex<WorkerState>>,
    ) -> RunpodRemoteWorkspaceProvider<
        FakeStore,
        FakeIdentityProvider,
        FakeStore,
        FakeIdentityProvider,
    > {
        let runpod_store = FakeStore::default();
        runpod_store.insert(SecretKey::RunpodApiKey, "runpod-secret");
        let hugging_face_store = FakeStore::default();
        hugging_face_store.insert(SecretKey::HuggingFaceApiKey, "hf-secret");

        RunpodRemoteWorkspaceProvider::with_clients(
            Arc::new(FakeApi { state: api_state }),
            Arc::new(FakeWorker {
                state: worker_state,
            }),
            SecretsStorageService::new(runpod_store, FakeIdentityProvider, SecretKey::RunpodApiKey),
            SecretsStorageService::new(
                hugging_face_store,
                FakeIdentityProvider,
                SecretKey::HuggingFaceApiKey,
            ),
        )
    }

    #[tokio::test]
    async fn create_volume_builds_network_volume_request() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        let volume = provider
            .create_volume(CreateVolumeParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                size_bytes: 1_000_000_001,
                mount_path: "/workspace".to_string(),
            })
            .await
            .expect("volume");

        assert_eq!(volume.id, "volume");
        assert_eq!(
            api_state.lock().expect("api state").create_volume_requests,
            vec![CreateNetworkVolumeRequest {
                datacenter_id: "dc".to_string(),
                name: "luma-forge-workspace-volume".to_string(),
                size_gb: 2,
            }]
        );
    }

    #[tokio::test]
    async fn start_provisioner_derives_token_and_injects_hf_when_required() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        let snapshot = provider
            .start_provisioner(StartProvisionerParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_id: "volume".to_string(),
                provisioner_image_ref: "image".to_string(),
                mount_path: "/workspace".to_string(),
                requires_hugging_face_api_key: true,
            })
            .await
            .expect("provisioner");

        assert_eq!(
            snapshot,
            RemoteProvisionerSnapshot {
                id: "pod".to_string(),
                status_url: "https://pod-8000.proxy.runpod.net/status".to_string(),
            }
        );
        let request = &api_state
            .lock()
            .expect("api state")
            .provisioner_pod_requests[0];
        assert_eq!(request.hugging_face_api_key, Some("hf-secret".to_string()));
        assert_eq!(request.bearer_token.len(), 64);
    }

    #[tokio::test]
    async fn start_provisioner_omits_hf_when_not_required() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        provider
            .start_provisioner(StartProvisionerParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_id: "volume".to_string(),
                provisioner_image_ref: "image".to_string(),
                mount_path: "/workspace".to_string(),
                requires_hugging_face_api_key: false,
            })
            .await
            .expect("provisioner");

        assert_eq!(
            api_state
                .lock()
                .expect("api state")
                .provisioner_pod_requests[0]
                .hugging_face_api_key,
            None
        );
    }

    #[tokio::test]
    async fn get_provisioner_status_maps_worker_unauthorized_to_workspace_worker_error() {
        let worker_state = Arc::new(Mutex::new(WorkerState {
            result: Some(Err(RemoteProvisioningError::ProvisionerWorkerUnauthorized)),
            ..WorkerState::default()
        }));
        let provider = provider(Arc::default(), Arc::clone(&worker_state));

        let result = provider
            .get_provisioner_status(GetProvisionerStatusParams {
                workspace_id: "workspace".to_string(),
                provisioner_id: "pod".to_string(),
                status_url: "https://status.example".to_string(),
            })
            .await;

        assert_eq!(
            result,
            Err(RemoteWorkspaceError::ProvisionerWorker(
                RemoteProvisioningError::ProvisionerWorkerUnauthorized
            ))
        );
        assert_eq!(
            worker_state.lock().expect("worker state").calls[0].0,
            "https://status.example"
        );
    }

    #[tokio::test]
    async fn create_endpoint_uses_default_keep_alive_limits_when_missing() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        provider
            .create_endpoint(CreateEndpointParams {
                workspace_id: "workspace".to_string(),
                datacenter_id: "dc".to_string(),
                gpu_id: "gpu".to_string(),
                volume_id: "volume".to_string(),
                endpoint_image_ref: "image".to_string(),
                mount_path: "/workspace".to_string(),
                keep_alive_limits: None,
            })
            .await
            .expect("endpoint");

        assert_eq!(
            api_state.lock().expect("api state").endpoint_requests[0].keep_alive_limits,
            RemoteEndpointKeepAliveLimits {
                default_seconds: 300,
                min_seconds: 0,
                max_seconds: 86_400,
            }
        );
    }

    #[tokio::test]
    async fn delete_endpoint_delegates_endpoint_and_template_cleanup() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        provider
            .delete_endpoint(DeleteEndpointParams {
                workspace_id: "workspace".to_string(),
                endpoint_id: "endpoint".to_string(),
            })
            .await
            .expect("delete endpoint");

        assert_eq!(
            api_state.lock().expect("api state").deleted_endpoints,
            vec!["endpoint".to_string()]
        );
    }
}
