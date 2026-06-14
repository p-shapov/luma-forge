use std::sync::Arc;

use crate::{
    domain::{runpod::RunpodPlacementOptions, workflow_preset::ModelAsset},
    shared::AppFuture,
};
use tracing::Instrument;

use super::super::errors::{
    hugging_face_api_key_unavailable, runpod_api_key_unavailable, RunpodRuntimeError,
};
use crate::secrets_storage::{ApiKeyIdentityProvider, SecretStore, SecretsStorageService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodEndpointKeepAliveLimits {
    pub default_seconds: u32,
    pub min_seconds: u32,
    pub max_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunpodProvisionerStatus {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodNetworkVolumeParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRunpodProvisionerPodParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub requires_hugging_face_api_key: bool,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessTemplateParams {
    pub workspace_id: String,
    pub endpoint_image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessEndpointParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub network_volume_id: String,
    pub template_id: String,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}

pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>>;

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;
}

use super::{
    api::{
        CreateNetworkVolumeRequest, CreateProvisionerPodRequest, CreateServerlessEndpointRequest,
        CreateServerlessTemplateRequest, HttpRunpodApi, RunpodApi,
    },
    config::{
        DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS, ENDPOINT_WORKSPACE_MOUNT_PATH,
        NETWORK_VOLUME_MAX_SIZE_GB, PROVISIONER_PORT, PROVISIONER_WORKSPACE_MOUNT_PATH,
        RUNPOD_GRAPHQL_URL, RUNPOD_REST_BASE_URL,
    },
    mapping,
    provisioner::{ProvisionerWorkerApi, ProvisionerWorkerClient},
};

pub struct RunpodRuntimeProvider<RS, RI, HS, HI> {
    api: Arc<dyn RunpodApi>,
    provisioner_worker: Arc<dyn ProvisionerWorkerApi>,
    runpod_secrets: Arc<SecretsStorageService<RS, RI>>,
    hugging_face_secrets: SecretsStorageService<HS, HI>,
}

impl<RS, RI, HS, HI> RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore + 'static,
    RI: ApiKeyIdentityProvider + 'static,
    HS: SecretStore + 'static,
    HI: ApiKeyIdentityProvider + 'static,
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

impl<RS, RI, HS, HI> RunpodRuntimeClient for RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>> {
        Box::pin(
            async move {
                let mut options = self.api.placement_options().await?;
                options.max_volume_size_gb = Some(NETWORK_VOLUME_MAX_SIZE_GB);
                Ok(options)
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "placement_options"
            )),
        )
    }

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let volume_size_gb = params.size_gb;
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_network_volume",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            volume_size_gb = volume_size_gb
        );
        Box::pin(
            async move {
                self.api
                    .create_network_volume(network_volume_request(params))
                    .await
            }
            .instrument(span),
        )
    }

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.api.delete_network_volume(network_volume_id).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "delete_network_volume",
                    network_volume_id = %network_volume_id
                ),
            ),
        )
    }

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let network_volume_id = params.network_volume_id.clone();
        let requires_hugging_face_api_key = params.requires_hugging_face_api_key;
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "start_provisioner_pod",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            network_volume_id = %network_volume_id,
            requires_hugging_face_api_key = requires_hugging_face_api_key
        );
        Box::pin(
            async move {
                let bearer_token = self.workspace_bearer_token(&params.workspace_id).await?;
                let hugging_face_api_key = self.hugging_face_api_key(&params).await?;
                let pod = self
                    .api
                    .create_provisioner_pod(provisioner_pod_request(
                        params,
                        bearer_token,
                        hugging_face_api_key,
                    ))
                    .await?;

                Ok(pod.id)
            }
            .instrument(span),
        )
    }

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.api.delete_provisioner_pod(provisioner_pod_id).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "terminate_provisioner_pod",
                    provisioner_pod_id = %provisioner_pod_id
                ),
            ),
        )
    }

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>> {
        Box::pin(
            async move {
                let bearer_token = self.workspace_bearer_token(workspace_id).await?;

                self.provisioner_worker
                    .get_status(&provisioner_status_url(provisioner_pod_id), &bearer_token)
                    .await
            }
            .instrument(tracing::info_span!(
                "runpod_provider",
                provider_operation = "get_provisioner_status",
                workspace_id = %workspace_id,
                provisioner_pod_id = %provisioner_pod_id
            )),
        )
    }

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_serverless_template",
            workspace_id = %workspace_id
        );
        Box::pin(
            async move {
                let template = self
                    .api
                    .create_serverless_template(serverless_template_request(params))
                    .await?;

                Ok(template.id)
            }
            .instrument(span),
        )
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
        let workspace_id = params.workspace_id.clone();
        let datacenter_id = params.data_center_id.clone();
        let gpu_type_id = params.gpu_type_id.clone();
        let network_volume_id = params.network_volume_id.clone();
        let template_id = params.template_id.clone();
        let span = tracing::info_span!(
            "runpod_provider",
            provider_operation = "create_serverless_endpoint",
            workspace_id = %workspace_id,
            datacenter_id = %datacenter_id,
            gpu_type_id = %gpu_type_id,
            network_volume_id = %network_volume_id,
            template_id = %template_id
        );
        Box::pin(
            async move {
                let endpoint = self
                    .api
                    .create_serverless_endpoint(serverless_endpoint_request(params))
                    .await?;

                Ok(endpoint.id)
            }
            .instrument(span),
        )
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.api.delete_endpoint(endpoint_id).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "delete_serverless_endpoint",
                    endpoint_id = %endpoint_id
                ),
            ),
        )
    }

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
        Box::pin(
            async move { self.api.delete_template(template_id).await }.instrument(
                tracing::info_span!(
                    "runpod_provider",
                    provider_operation = "delete_template",
                    template_id = %template_id
                ),
            ),
        )
    }
}

impl<RS, RI, HS, HI> RunpodRuntimeProvider<RS, RI, HS, HI>
where
    RS: SecretStore,
    RI: ApiKeyIdentityProvider,
    HS: SecretStore,
    HI: ApiKeyIdentityProvider,
{
    async fn workspace_bearer_token(
        &self,
        workspace_id: &str,
    ) -> Result<String, RunpodRuntimeError> {
        self.runpod_secrets
            .hmac_sha256_hex(workspace_id)
            .await
            .map_err(runpod_api_key_unavailable)
    }

    async fn hugging_face_api_key(
        &self,
        params: &StartRunpodProvisionerPodParams,
    ) -> Result<Option<String>, RunpodRuntimeError> {
        if !params.requires_hugging_face_api_key {
            return Ok(None);
        }

        self.hugging_face_secrets
            .retrieve()
            .await
            .map_err(hugging_face_api_key_unavailable)
            .map(|secret| Some(secret.expose_secret().to_string()))
    }
}

fn network_volume_request(params: CreateRunpodNetworkVolumeParams) -> CreateNetworkVolumeRequest {
    CreateNetworkVolumeRequest {
        datacenter_id: params.data_center_id,
        name: mapping::network_volume_name(&params.workspace_id),
        size_gb: params.size_gb,
    }
}

fn provisioner_pod_request(
    params: StartRunpodProvisionerPodParams,
    bearer_token: String,
    hugging_face_api_key: Option<String>,
) -> CreateProvisionerPodRequest {
    CreateProvisionerPodRequest {
        datacenter_id: params.data_center_id,
        name: mapping::provisioner_pod_name(&params.workspace_id),
        image_ref: params.provisioner_image_ref,
        network_volume_id: params.network_volume_id,
        mount_path: PROVISIONER_WORKSPACE_MOUNT_PATH.to_string(),
        bearer_token,
        required_model_assets: params.required_model_assets,
        hugging_face_api_key,
    }
}

fn serverless_template_request(
    params: CreateRunpodServerlessTemplateParams,
) -> CreateServerlessTemplateRequest {
    CreateServerlessTemplateRequest {
        name: mapping::endpoint_template_name(&params.workspace_id),
        image_ref: params.endpoint_image_ref,
        mount_path: ENDPOINT_WORKSPACE_MOUNT_PATH.to_string(),
    }
}

fn serverless_endpoint_request(
    params: CreateRunpodServerlessEndpointParams,
) -> CreateServerlessEndpointRequest {
    CreateServerlessEndpointRequest {
        datacenter_id: params.data_center_id,
        gpu_id: params.gpu_type_id,
        name: mapping::endpoint_name(&params.workspace_id),
        template_id: params.template_id,
        network_volume_id: params.network_volume_id,
        keep_alive_limits: params
            .keep_alive_limits
            .unwrap_or(DEFAULT_ENDPOINT_KEEP_ALIVE_LIMITS),
    }
}

fn provisioner_status_url(pod_id: &str) -> String {
    format!("https://{pod_id}-{PROVISIONER_PORT}.proxy.runpod.net/status")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use crate::secrets_storage::SecretsStorageError;
    use crate::{
        domain::{
            runpod::{RunpodDatacenterPlacementOption, RunpodGpuPlacementOption},
            secrets::ApiKeyIdentity,
        },
        secrets_storage::{ApiSecret, SecretKey, SecretStore},
    };

    use super::super::api::{RunpodEndpoint, RunpodId};
    use super::super::config::ENDPOINT_WORKSPACE_MOUNT_PATH;
    use super::*;
    use crate::domain::workflow_preset::{ModelAsset, ModelAssetSource};

    #[derive(Default)]
    struct ApiState {
        create_network_volume_requests: Vec<CreateNetworkVolumeRequest>,
        provisioner_pod_requests: Vec<CreateProvisionerPodRequest>,
        template_requests: Vec<CreateServerlessTemplateRequest>,
        endpoint_requests: Vec<CreateServerlessEndpointRequest>,
        create_serverless_endpoint_error: Option<RunpodRuntimeError>,
        deleted_endpoints: Vec<String>,
        deleted_templates: Vec<String>,
    }

    struct FakeApi {
        state: Arc<Mutex<ApiState>>,
    }

    impl RunpodApi for FakeApi {
        fn placement_options<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>> {
            Box::pin(async {
                Ok(RunpodPlacementOptions {
                    max_volume_size_gb: None,
                    datacenters: vec![RunpodDatacenterPlacementOption {
                        id: "dc".to_string(),
                        name: "Datacenter".to_string(),
                        gpu_options: vec![RunpodGpuPlacementOption {
                            id: "gpu".to_string(),
                            name: "GPU".to_string(),
                            vram_gb: 24,
                        }],
                    }],
                })
            })
        }

        fn create_network_volume<'a>(
            &'a self,
            request: CreateNetworkVolumeRequest,
        ) -> AppFuture<'a, Result<String, RunpodRuntimeError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .create_network_volume_requests
                    .push(request);
                Ok("volume".to_string())
            })
        }

        fn delete_network_volume<'a>(
            &'a self,
            _volume_id: &'a str,
        ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
            Box::pin(async { Ok(()) })
        }

        fn create_provisioner_pod<'a>(
            &'a self,
            request: CreateProvisionerPodRequest,
        ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>> {
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
        ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
            Box::pin(async { Ok(()) })
        }

        fn create_serverless_template<'a>(
            &'a self,
            request: CreateServerlessTemplateRequest,
        ) -> AppFuture<'a, Result<RunpodId, RunpodRuntimeError>> {
            Box::pin(async move {
                self.state
                    .lock()
                    .expect("api state")
                    .template_requests
                    .push(request);
                Ok(RunpodId {
                    id: "template".to_string(),
                })
            })
        }

        fn create_serverless_endpoint<'a>(
            &'a self,
            request: CreateServerlessEndpointRequest,
        ) -> AppFuture<'a, Result<RunpodEndpoint, RunpodRuntimeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("api state");
                state.endpoint_requests.push(request);
                if let Some(error) = state.create_serverless_endpoint_error.clone() {
                    return Err(error);
                }

                Ok(RunpodEndpoint {
                    id: "endpoint".to_string(),
                    template_id: "template".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            endpoint_id: &'a str,
        ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("api state");
                state.deleted_endpoints.push(endpoint_id.to_string());
                Ok(())
            })
        }

        fn delete_template<'a>(
            &'a self,
            template_id: &'a str,
        ) -> AppFuture<'a, Result<(), RunpodRuntimeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("api state");
                state.deleted_templates.push(template_id.to_string());
                Ok(())
            })
        }
    }

    #[derive(Default)]
    struct WorkerState {
        calls: Vec<(String, String)>,
        result: Option<Result<RunpodProvisionerStatus, RunpodRuntimeError>>,
    }

    struct FakeWorker {
        state: Arc<Mutex<WorkerState>>,
    }

    impl ProvisionerWorkerApi for FakeWorker {
        fn get_status<'a>(
            &'a self,
            status_url: &'a str,
            bearer_token: &'a str,
        ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>> {
            Box::pin(async move {
                let mut state = self.state.lock().expect("worker state");
                state
                    .calls
                    .push((status_url.to_string(), bearer_token.to_string()));
                state
                    .result
                    .clone()
                    .unwrap_or(Ok(RunpodProvisionerStatus::Running))
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
    ) -> RunpodRuntimeProvider<FakeStore, FakeIdentityProvider, FakeStore, FakeIdentityProvider>
    {
        let runpod_store = FakeStore::default();
        runpod_store.insert(SecretKey::RunpodApiKey, "runpod-secret");
        let hugging_face_store = FakeStore::default();
        hugging_face_store.insert(SecretKey::HuggingFaceApiKey, "hf-secret");

        RunpodRuntimeProvider::with_clients(
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
    async fn create_network_volume_builds_network_volume_request() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        let volume = provider
            .create_network_volume(CreateRunpodNetworkVolumeParams {
                workspace_id: "workspace".to_string(),
                data_center_id: "dc".to_string(),
                size_gb: 75,
            })
            .await
            .expect("volume");

        assert_eq!(volume, "volume");
        assert_eq!(
            api_state
                .lock()
                .expect("api state")
                .create_network_volume_requests,
            vec![CreateNetworkVolumeRequest {
                datacenter_id: "dc".to_string(),
                name: "luma-forge-workspace-volume".to_string(),
                size_gb: 75,
            }]
        );
    }

    #[tokio::test]
    async fn placement_options_sets_max_network_volume_size_in_gb() {
        let provider = provider(Arc::default(), Arc::default());

        let options = provider
            .placement_options()
            .await
            .expect("placement options");

        assert_eq!(options.max_volume_size_gb, Some(4_000));
    }

    #[tokio::test]
    async fn start_provisioner_pod_derives_token_and_injects_hf_when_workflow_requires_it() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        let provisioner_id = provider
            .start_provisioner_pod(StartRunpodProvisionerPodParams {
                workspace_id: "workspace".to_string(),
                data_center_id: "dc".to_string(),
                network_volume_id: "volume".to_string(),
                provisioner_image_ref: "image".to_string(),
                requires_hugging_face_api_key: true,
                required_model_assets: vec![ModelAsset {
                    id: "model".to_string(),
                    name: "Model".to_string(),
                    download_source: ModelAssetSource::Huggingface {
                        repository_id: "owner/model".to_string(),
                        file_path: "model.safetensors".to_string(),
                        revision: "main".to_string(),
                    },
                    install_comfyui_relative_path: "models/checkpoints/model.safetensors"
                        .to_string(),
                }],
            })
            .await
            .expect("provisioner");

        assert_eq!(provisioner_id, "pod");
        let request = &api_state
            .lock()
            .expect("api state")
            .provisioner_pod_requests[0];
        assert_eq!(request.name, "luma-forge-workspace-provisioner");
        assert_eq!(request.hugging_face_api_key, Some("hf-secret".to_string()));
        assert_eq!(
            request.required_model_assets,
            vec![ModelAsset {
                id: "model".to_string(),
                name: "Model".to_string(),
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/model".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                install_comfyui_relative_path: "models/checkpoints/model.safetensors".to_string(),
            }]
        );
        assert_eq!(request.bearer_token.len(), 64);
    }

    #[tokio::test]
    async fn start_provisioner_pod_omits_hf_when_not_required() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        provider
            .start_provisioner_pod(StartRunpodProvisionerPodParams {
                workspace_id: "workspace".to_string(),
                data_center_id: "dc".to_string(),
                network_volume_id: "volume".to_string(),
                provisioner_image_ref: "image".to_string(),
                requires_hugging_face_api_key: false,
                required_model_assets: Vec::new(),
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
    async fn get_provisioner_status_maps_worker_auth_failure_to_provisioner_error() {
        let worker_state = Arc::new(Mutex::new(WorkerState {
            result: Some(Err(RunpodRuntimeError::ProvisionerWorkerResponseInvalid {
                message: "response invalid".to_string(),
            })),
            ..WorkerState::default()
        }));
        let provider = provider(Arc::default(), Arc::clone(&worker_state));

        let result = provider.get_provisioner_status("workspace", "pod").await;

        assert!(matches!(
            result,
            Err(RunpodRuntimeError::ProvisionerWorkerResponseInvalid { .. })
        ));
        assert_eq!(
            worker_state.lock().expect("worker state").calls[0].0,
            "https://pod-8000.proxy.runpod.net/status"
        );
    }

    #[tokio::test]
    async fn creates_serverless_template_then_endpoint_from_template_id() {
        let api_state = Arc::new(Mutex::new(ApiState::default()));
        let provider = provider(Arc::clone(&api_state), Arc::default());

        let template_id = provider
            .create_serverless_template(CreateRunpodServerlessTemplateParams {
                workspace_id: "workspace".to_string(),
                endpoint_image_ref: "image".to_string(),
            })
            .await
            .expect("template");
        let endpoint_id = provider
            .create_serverless_endpoint(CreateRunpodServerlessEndpointParams {
                workspace_id: "workspace".to_string(),
                data_center_id: "dc".to_string(),
                gpu_type_id: "gpu".to_string(),
                network_volume_id: "volume".to_string(),
                template_id: template_id.clone(),
                keep_alive_limits: None,
            })
            .await
            .expect("endpoint");

        assert_eq!(template_id, "template");
        assert_eq!(endpoint_id, "endpoint");
        let state = api_state.lock().expect("api state");
        let template_request = &state.template_requests[0];
        let endpoint_request = &state.endpoint_requests[0];
        assert_eq!(template_request.mount_path, ENDPOINT_WORKSPACE_MOUNT_PATH);
        assert_eq!(
            template_request.name,
            "luma-forge-workspace-endpoint-template"
        );
        assert_eq!(endpoint_request.name, "luma-forge-workspace-endpoint");
        assert_eq!(endpoint_request.template_id, "template");
        assert_eq!(
            endpoint_request.keep_alive_limits,
            RunpodEndpointKeepAliveLimits {
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
            .delete_serverless_endpoint("endpoint")
            .await
            .expect("delete endpoint");
        provider
            .delete_template("template")
            .await
            .expect("delete template");

        let state = api_state.lock().expect("api state");
        assert_eq!(state.deleted_endpoints, vec!["endpoint".to_string()]);
        assert_eq!(state.deleted_templates, vec!["template".to_string()]);
    }
}
