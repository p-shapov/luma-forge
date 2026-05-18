use super::*;

use std::{
    collections::HashMap,
    future::Future,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    sync::{Arc, Mutex, OnceLock},
    thread,
};

use serde_json::{json, Value};

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderProvisioningSnapshot, ProviderResourceStatus,
            ProvisioningPodSnapshot, RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot,
            Workspace, WorkspaceCatalog, WorkspaceLifecycleState, WorkspaceProvisioningFailureCode,
            WorkspaceProvisioningFailureSource, WorkspaceProvisioningPhase,
            WorkspaceProvisioningRecoveryAction,
        },
    },
    provisioner_worker::{
        ProvisionerWorkerError, ProvisionerWorkerGateway, ProvisionerWorkerJobStatus,
        ProvisionerWorkerStartRequest, ProvisionerWorkerStatus,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError},
    workspace_catalog::repository::WorkspaceCatalogRepository,
    workspace_resources::{
        provider_resource_name, CreateEndpointTemplateInput, CreateNetworkVolumeInput,
        CreateProvisioningPodInput, EndpointTemplateObservation, NetworkVolumeObservation,
        ProvisioningPodObservation, ServerlessEndpointObservation, WorkspaceResourceError,
        WorkspaceResourceService,
    },
    workspace_setup::{
        error::WorkspaceSetupError,
        tests::{sample_runtime_snapshot, sample_workspace},
    },
};

#[derive(Debug, Clone)]
struct MemorySecretStore {
    api_key: Option<String>,
    worker_tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl SecretStore for MemorySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(self.api_key.is_some())
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        self.api_key
            .clone()
            .map(ProviderApiKey::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provisioning tests do not replace provider keys")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("provisioning tests do not delete provider keys")
    }

    fn write_provisioner_worker_token(
        &self,
        workspace_id: &str,
        token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .insert(workspace_id.to_string(), token.expose_secret().to_string());
        Ok(())
    }

    fn read_provisioner_worker_token(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .get(workspace_id)
            .cloned()
            .map(ProvisionerWorkerBearerToken::new)
            .transpose()
            .map_err(|_| SecretStoreError::InvalidStoredProvisionerWorkerToken)
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        self.worker_tokens
            .lock()
            .expect("worker token lock")
            .remove(workspace_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct MemoryWorkspaceCatalog {
    workspaces: Arc<Mutex<Vec<Workspace>>>,
}

impl WorkspaceCatalogRepository for MemoryWorkspaceCatalog {
    fn list_workspaces<'a>(
        &'a self,
    ) -> Pin<Box<dyn Future<Output = Result<WorkspaceCatalog, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            Ok(WorkspaceCatalog {
                workspaces: self.workspaces.lock().expect("catalog lock").clone(),
            })
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Workspace>, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            Ok(self
                .workspaces
                .lock()
                .expect("catalog lock")
                .iter()
                .find(|workspace| workspace.id == id)
                .cloned())
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            self.workspaces
                .lock()
                .expect("catalog lock")
                .push(workspace.clone());
            Ok(workspace.clone())
        })
    }

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> Pin<Box<dyn Future<Output = Result<Workspace, WorkspaceSetupError>> + Send + 'a>> {
        Box::pin(async move {
            let mut workspaces = self.workspaces.lock().expect("catalog lock");
            let existing = workspaces
                .iter_mut()
                .find(|existing| existing.id == workspace.id)
                .ok_or(WorkspaceSetupError::WorkspaceCatalogQueryFailed)?;
            *existing = workspace.clone();
            Ok(workspace.clone())
        })
    }
}

#[derive(Debug, Clone)]
struct FakeProvider {
    api_key: String,
    create_volume_count: Arc<AtomicUsize>,
    create_volume_inputs: Arc<Mutex<Vec<CreateNetworkVolumeInput>>>,
    get_volume_count: Arc<AtomicUsize>,
    delete_volume_count: Arc<AtomicUsize>,
    discover_volumes_count: Arc<AtomicUsize>,
    discover_pods_count: Arc<AtomicUsize>,
    create_pod_count: Arc<AtomicUsize>,
    create_pod_inputs: Arc<Mutex<Vec<CreateProvisioningPodInput>>>,
    get_pod_count: Arc<AtomicUsize>,
    delete_pod_count: Arc<AtomicUsize>,
    discover_templates_count: Arc<AtomicUsize>,
    create_template_count: Arc<AtomicUsize>,
    create_template_inputs: Arc<Mutex<Vec<CreateEndpointTemplateInput>>>,
    get_template_count: Arc<AtomicUsize>,
    delete_template_count: Arc<AtomicUsize>,
    discover_endpoints_count: Arc<AtomicUsize>,
    create_endpoint_count: Arc<AtomicUsize>,
    get_endpoint_count: Arc<AtomicUsize>,
    delete_endpoint_count: Arc<AtomicUsize>,
    create_volume_error: Option<WorkspaceResourceError>,
    create_pod_error: Option<WorkspaceResourceError>,
    create_template_error: Option<WorkspaceResourceError>,
    create_endpoint_error: Option<WorkspaceResourceError>,
    get_volume_error: Option<WorkspaceResourceError>,
    get_pod_error: Option<WorkspaceResourceError>,
    get_template_error: Option<WorkspaceResourceError>,
    get_endpoint_error: Option<WorkspaceResourceError>,
    discovered_volumes: Vec<NetworkVolumeObservation>,
    subsequent_discovered_volumes: Option<Vec<NetworkVolumeObservation>>,
    discovered_pods: Vec<ProvisioningPodObservation>,
    subsequent_discovered_pods: Option<Vec<ProvisioningPodObservation>>,
    discovered_templates: Vec<EndpointTemplateObservation>,
    subsequent_discovered_templates: Option<Vec<EndpointTemplateObservation>>,
    discovered_endpoints: Vec<ServerlessEndpointObservation>,
    subsequent_discovered_endpoints: Option<Vec<ServerlessEndpointObservation>>,
    get_volume_status: Option<ProviderResourceStatus>,
    get_pod_status_url: Option<Option<String>>,
    get_template_status: Option<ProviderResourceStatus>,
    get_endpoint_status: Option<ProviderResourceStatus>,
    delete_endpoint_error: Option<WorkspaceResourceError>,
}

impl Default for FakeProvider {
    fn default() -> Self {
        static NEXT_PROVIDER_ID: AtomicUsize = AtomicUsize::new(1);

        let provider_id = NEXT_PROVIDER_ID.fetch_add(1, Ordering::SeqCst);
        Self {
            api_key: format!("rp_test_{provider_id}_secret"),
            create_volume_count: Arc::default(),
            create_volume_inputs: Arc::default(),
            get_volume_count: Arc::default(),
            delete_volume_count: Arc::default(),
            discover_volumes_count: Arc::default(),
            discover_pods_count: Arc::default(),
            create_pod_count: Arc::default(),
            create_pod_inputs: Arc::default(),
            get_pod_count: Arc::default(),
            delete_pod_count: Arc::default(),
            discover_templates_count: Arc::default(),
            create_template_count: Arc::default(),
            create_template_inputs: Arc::default(),
            get_template_count: Arc::default(),
            delete_template_count: Arc::default(),
            discover_endpoints_count: Arc::default(),
            create_endpoint_count: Arc::default(),
            get_endpoint_count: Arc::default(),
            delete_endpoint_count: Arc::default(),
            create_volume_error: None,
            create_pod_error: None,
            create_template_error: None,
            create_endpoint_error: None,
            get_volume_error: None,
            get_pod_error: None,
            get_template_error: None,
            get_endpoint_error: None,
            discovered_volumes: Vec::new(),
            subsequent_discovered_volumes: None,
            discovered_pods: Vec::new(),
            subsequent_discovered_pods: None,
            discovered_templates: Vec::new(),
            subsequent_discovered_templates: None,
            discovered_endpoints: Vec::new(),
            subsequent_discovered_endpoints: None,
            get_volume_status: None,
            get_pod_status_url: None,
            get_template_status: None,
            get_endpoint_status: None,
            delete_endpoint_error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct RegisteredFakeProvider {
    provider: FakeProvider,
    workspace_id: String,
}

#[derive(Debug)]
struct FakeRunPodServer {
    providers: Arc<Mutex<HashMap<String, RegisteredFakeProvider>>>,
}

static FAKE_RUNPOD_SERVER: OnceLock<FakeRunPodServer> = OnceLock::new();

fn register_fake_provider(catalog: &MemoryWorkspaceCatalog, provider: &FakeProvider) {
    let server = FAKE_RUNPOD_SERVER.get_or_init(start_fake_runpod_server);
    let workspace_id = catalog
        .workspaces
        .lock()
        .expect("catalog lock")
        .first()
        .map(|workspace| workspace.id.clone())
        .unwrap_or_else(|| "018f6a40-0000-7000-8000-000000000001".to_string());

    server.providers.lock().expect("providers lock").insert(
        provider.api_key.clone(),
        RegisteredFakeProvider {
            provider: provider.clone(),
            workspace_id,
        },
    );
}

fn start_fake_runpod_server() -> FakeRunPodServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake RunPod server");
    let endpoint = format!(
        "http://{}",
        listener
            .local_addr()
            .expect("fake RunPod server local address")
    );
    crate::provider::runpod::set_default_test_rest_endpoint(endpoint);

    let providers = Arc::new(Mutex::new(HashMap::<String, RegisteredFakeProvider>::new()));
    let server_providers = providers.clone();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let providers = server_providers.clone();
            thread::spawn(move || handle_fake_runpod_connection(stream, providers));
        }
    });

    FakeRunPodServer { providers }
}

#[derive(Debug)]
struct FakeHttpRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Vec<u8>,
}

fn handle_fake_runpod_connection(
    mut stream: TcpStream,
    providers: Arc<Mutex<HashMap<String, RegisteredFakeProvider>>>,
) {
    let Some(request) = read_fake_http_request(&mut stream) else {
        return;
    };
    let (status, body) = handle_fake_runpod_request(request, providers);
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn read_fake_http_request(stream: &mut TcpStream) -> Option<FakeHttpRequest> {
    let mut buffer = Vec::new();
    let mut scratch = [0_u8; 1024];
    loop {
        let read = stream.read(&mut scratch).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&scratch[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n")?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();

    let mut authorization = None;
    let mut content_length = 0_usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.trim().to_string());
        } else if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().unwrap_or_default();
        }
    }

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut scratch).ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&scratch[..read]);
    }

    Some(FakeHttpRequest {
        method,
        path,
        authorization,
        body: buffer
            .get(body_start..body_start + content_length)
            .unwrap_or_default()
            .to_vec(),
    })
}

fn handle_fake_runpod_request(
    request: FakeHttpRequest,
    providers: Arc<Mutex<HashMap<String, RegisteredFakeProvider>>>,
) -> (u16, Value) {
    let Some(api_key) = request
        .authorization
        .as_deref()
        .and_then(|value| value.strip_prefix("Bearer "))
    else {
        return (401, json!({}));
    };

    let Some(registered) = providers
        .lock()
        .expect("providers lock")
        .get(api_key)
        .cloned()
    else {
        return (401, json!({}));
    };

    fake_runpod_response(request, registered)
}

fn fake_runpod_response(
    request: FakeHttpRequest,
    registered: RegisteredFakeProvider,
) -> (u16, Value) {
    let path = request.path.trim_start_matches('/');
    let segments = path.split('/').collect::<Vec<_>>();
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", ["networkvolumes"]) => fake_discover_volumes(&registered),
        ("POST", ["networkvolumes"]) => fake_create_volume(&request.body, &registered),
        ("GET", ["networkvolumes", volume_id]) => fake_get_volume(volume_id, &registered.provider),
        ("DELETE", ["networkvolumes", _]) => {
            registered
                .provider
                .delete_volume_count
                .fetch_add(1, Ordering::SeqCst);
            (200, json!({}))
        }
        ("GET", ["pods"]) => fake_discover_pods(&registered),
        ("POST", ["pods"]) => fake_create_pod(&request.body, &registered),
        ("GET", ["pods", pod_id]) => fake_get_pod(pod_id, &registered.provider),
        ("DELETE", ["pods", _]) => {
            registered
                .provider
                .delete_pod_count
                .fetch_add(1, Ordering::SeqCst);
            (200, json!({}))
        }
        ("GET", ["templates"]) => fake_discover_templates(&registered),
        ("POST", ["templates"]) => fake_create_template(&request.body, &registered),
        ("GET", ["templates", template_id]) => fake_get_template(template_id, &registered.provider),
        ("DELETE", ["templates", _]) => {
            registered
                .provider
                .delete_template_count
                .fetch_add(1, Ordering::SeqCst);
            (200, json!({}))
        }
        ("GET", ["endpoints"]) => fake_discover_endpoints(&registered),
        ("POST", ["endpoints"]) => fake_create_endpoint(&registered),
        ("GET", ["endpoints", endpoint_id]) => fake_get_endpoint(endpoint_id, &registered.provider),
        ("DELETE", ["endpoints", _]) => {
            registered
                .provider
                .delete_endpoint_count
                .fetch_add(1, Ordering::SeqCst);
            error_response(registered.provider.delete_endpoint_error.as_ref())
                .unwrap_or((200, json!({})))
        }
        _ => (404, json!({})),
    }
}

fn fake_discover_volumes(registered: &RegisteredFakeProvider) -> (u16, Value) {
    let call = registered
        .provider
        .discover_volumes_count
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let observations = if call > 1 {
        registered
            .provider
            .subsequent_discovered_volumes
            .as_ref()
            .unwrap_or(&registered.provider.discovered_volumes)
    } else {
        &registered.provider.discovered_volumes
    };
    let name = provider_resource_name(&registered.workspace_id, "volume");
    (
        200,
        Value::Array(
            observations
                .iter()
                .map(|observation| {
                    json!({
                        "id": observation.provider_resource_id,
                        "name": name,
                        "status": runpod_status(&observation.provider_resource_status),
                    })
                })
                .collect(),
        ),
    )
}

fn fake_create_volume(body: &[u8], registered: &RegisteredFakeProvider) -> (u16, Value) {
    registered
        .provider
        .create_volume_count
        .fetch_add(1, Ordering::SeqCst);
    let payload = serde_json::from_slice::<Value>(body).unwrap_or_else(|_| json!({}));
    registered
        .provider
        .create_volume_inputs
        .lock()
        .expect("volume inputs")
        .push(CreateNetworkVolumeInput {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            workspace_id: registered.workspace_id.clone(),
            datacenter_id: payload
                .get("dataCenterId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            size_bytes: payload
                .get("size")
                .and_then(Value::as_u64)
                .unwrap_or_default()
                * 1024
                * 1024
                * 1024,
        });
    if let Some(response) = error_response(registered.provider.create_volume_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": "volume-1",
            "name": provider_resource_name(&registered.workspace_id, "volume"),
            "status": "READY",
        }),
    )
}

fn fake_get_volume(volume_id: &str, provider: &FakeProvider) -> (u16, Value) {
    provider.get_volume_count.fetch_add(1, Ordering::SeqCst);
    if let Some(response) = error_response(provider.get_volume_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": volume_id,
            "name": "volume",
            "status": runpod_status(provider.get_volume_status.as_ref().unwrap_or(&ProviderResourceStatus::Ready)),
        }),
    )
}

fn fake_discover_pods(registered: &RegisteredFakeProvider) -> (u16, Value) {
    let call = registered
        .provider
        .discover_pods_count
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let observations = if call > 1 {
        registered
            .provider
            .subsequent_discovered_pods
            .as_ref()
            .unwrap_or(&registered.provider.discovered_pods)
    } else {
        &registered.provider.discovered_pods
    };
    let name = provider_resource_name(&registered.workspace_id, "provisioner");
    (
        200,
        Value::Array(
            observations
                .iter()
                .map(|observation| {
                    json!({
                        "id": observation.provider_resource_id,
                        "name": name,
                        "podStatus": runpod_status(&observation.provider_resource_status),
                    })
                })
                .collect(),
        ),
    )
}

fn fake_create_pod(body: &[u8], registered: &RegisteredFakeProvider) -> (u16, Value) {
    registered
        .provider
        .create_pod_count
        .fetch_add(1, Ordering::SeqCst);
    let payload = serde_json::from_slice::<Value>(body).unwrap_or_else(|_| json!({}));
    registered
        .provider
        .create_pod_inputs
        .lock()
        .expect("pod inputs")
        .push(CreateProvisioningPodInput {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            workspace_id: registered.workspace_id.clone(),
            provisioner_worker_image_ref: payload
                .get("imageName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            datacenter_id: payload
                .get("dataCenterIds")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            selected_gpu_id: payload
                .get("gpuTypeIds")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            network_volume_id: payload
                .get("networkVolumeId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            mount_path: payload
                .get("volumeMountPath")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            bearer_token: ProvisionerWorkerBearerToken::new(
                payload
                    .get("env")
                    .and_then(|env| env.get("LUMA_FORGE_PROVISIONER_BEARER_TOKEN"))
                    .and_then(Value::as_str)
                    .unwrap_or("worker-token")
                    .to_string(),
            )
            .expect("valid worker token"),
        });
    if let Some(response) = error_response(registered.provider.create_pod_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": "pod-1",
            "name": provider_resource_name(&registered.workspace_id, "provisioner"),
            "podStatus": "RUNNING",
            "ports": ["8000/tcp"],
            "publicIp": "203.0.113.10",
            "portMappings": { "8000": 30001 },
        }),
    )
}

fn fake_get_pod(pod_id: &str, provider: &FakeProvider) -> (u16, Value) {
    provider.get_pod_count.fetch_add(1, Ordering::SeqCst);
    if let Some(response) = error_response(provider.get_pod_error.as_ref()) {
        return response;
    }
    let mut payload = json!({
        "id": pod_id,
        "name": "pod",
        "podStatus": "RUNNING",
    });
    if provider.get_pod_status_url != Some(None) {
        payload["ports"] = json!(["8000/tcp"]);
        payload["publicIp"] = json!("203.0.113.10");
        payload["portMappings"] = json!({ "8000": 30001 });
    }
    (200, payload)
}

fn fake_discover_templates(registered: &RegisteredFakeProvider) -> (u16, Value) {
    let call = registered
        .provider
        .discover_templates_count
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let observations = if call > 1 {
        registered
            .provider
            .subsequent_discovered_templates
            .as_ref()
            .unwrap_or(&registered.provider.discovered_templates)
    } else {
        &registered.provider.discovered_templates
    };
    let name = provider_resource_name(&registered.workspace_id, "endpoint-template");
    (
        200,
        Value::Array(
            observations
                .iter()
                .map(|observation| {
                    json!({
                        "id": observation.template_id,
                        "name": name,
                        "imageName": observation.endpoint_worker_image_ref,
                        "isServerless": true,
                        "volumeMountPath": observation.mount_path,
                        "status": runpod_status(&observation.provider_resource_status),
                    })
                })
                .collect(),
        ),
    )
}

fn fake_create_template(body: &[u8], registered: &RegisteredFakeProvider) -> (u16, Value) {
    registered
        .provider
        .create_template_count
        .fetch_add(1, Ordering::SeqCst);
    let payload = serde_json::from_slice::<Value>(body).unwrap_or_else(|_| json!({}));
    registered
        .provider
        .create_template_inputs
        .lock()
        .expect("template inputs")
        .push(CreateEndpointTemplateInput {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            workspace_id: registered.workspace_id.clone(),
            endpoint_worker_image_ref: payload
                .get("imageName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            mount_path: payload
                .get("volumeMountPath")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    if let Some(response) = error_response(registered.provider.create_template_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": "template-1",
            "name": provider_resource_name(&registered.workspace_id, "endpoint-template"),
            "imageName": payload.get("imageName").and_then(Value::as_str).unwrap_or_default(),
            "isServerless": true,
            "volumeMountPath": payload.get("volumeMountPath").and_then(Value::as_str).unwrap_or("/workspace"),
            "status": "READY",
        }),
    )
}

fn fake_get_template(template_id: &str, provider: &FakeProvider) -> (u16, Value) {
    provider.get_template_count.fetch_add(1, Ordering::SeqCst);
    if let Some(response) = error_response(provider.get_template_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": template_id,
            "name": "template",
            "imageName": sample_endpoint_worker_image_ref(),
            "isServerless": true,
            "volumeMountPath": "/workspace",
            "status": runpod_status(provider.get_template_status.as_ref().unwrap_or(&ProviderResourceStatus::Ready)),
        }),
    )
}

fn fake_discover_endpoints(registered: &RegisteredFakeProvider) -> (u16, Value) {
    let call = registered
        .provider
        .discover_endpoints_count
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let observations = if call > 1 {
        registered
            .provider
            .subsequent_discovered_endpoints
            .as_ref()
            .unwrap_or(&registered.provider.discovered_endpoints)
    } else {
        &registered.provider.discovered_endpoints
    };
    let name = provider_resource_name(&registered.workspace_id, "endpoint");
    (
        200,
        Value::Array(
            observations
                .iter()
                .map(|observation| {
                    json!({
                        "id": observation.provider_resource_id,
                        "name": name,
                        "status": runpod_status(&observation.provider_resource_status),
                        "endpointUrl": observation.endpoint_invoke_url,
                    })
                })
                .collect(),
        ),
    )
}

fn fake_create_endpoint(registered: &RegisteredFakeProvider) -> (u16, Value) {
    registered
        .provider
        .create_endpoint_count
        .fetch_add(1, Ordering::SeqCst);
    if let Some(response) = error_response(registered.provider.create_endpoint_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": "endpoint-1",
            "name": provider_resource_name(&registered.workspace_id, "endpoint"),
            "status": "READY",
            "endpointUrl": "https://api.runpod.ai/v2/endpoint-1/runsync",
        }),
    )
}

fn fake_get_endpoint(endpoint_id: &str, provider: &FakeProvider) -> (u16, Value) {
    provider.get_endpoint_count.fetch_add(1, Ordering::SeqCst);
    if let Some(response) = error_response(provider.get_endpoint_error.as_ref()) {
        return response;
    }
    (
        200,
        json!({
            "id": endpoint_id,
            "name": "endpoint",
            "status": runpod_status(provider.get_endpoint_status.as_ref().unwrap_or(&ProviderResourceStatus::Ready)),
            "endpointUrl": format!("https://api.runpod.ai/v2/{endpoint_id}/runsync"),
        }),
    )
}

fn error_response(error: Option<&WorkspaceResourceError>) -> Option<(u16, Value)> {
    let status = match error? {
        WorkspaceResourceError::ProviderApiKeyUnauthorized => 401,
        WorkspaceResourceError::ProviderRateLimited => 429,
        WorkspaceResourceError::ProviderRequestRejected => 400,
        WorkspaceResourceError::ProviderResourceNotFound => 404,
        WorkspaceResourceError::ProviderOperationConflict => 409,
        WorkspaceResourceError::ProviderOperationIndeterminate => 408,
        WorkspaceResourceError::ProviderApiUnavailable => 500,
        WorkspaceResourceError::ProviderResponseInvalid => return Some((200, json!({}))),
        WorkspaceResourceError::WorkspaceCatalogUnavailable
        | WorkspaceResourceError::ProviderSetupIncomplete
        | WorkspaceResourceError::SecureKeyringUnavailable
        | WorkspaceResourceError::ProvisionerWorkerTokenInvalid => 500,
    };
    Some((status, json!({})))
}

fn runpod_status(status: &ProviderResourceStatus) -> &'static str {
    match status {
        ProviderResourceStatus::Ready => "READY",
        ProviderResourceStatus::Running => "RUNNING",
        ProviderResourceStatus::Creating => "CREATING",
        ProviderResourceStatus::Failed => "FAILED",
        ProviderResourceStatus::Terminated => "TERMINATED",
        ProviderResourceStatus::Unknown => "UNKNOWN",
    }
}

type TestWorkspaceProvisioningService =
    WorkspaceProvisioningService<MemorySecretStore, MemoryWorkspaceCatalog, FakeWorker>;

#[derive(Debug, Clone)]
struct FakeWorker {
    start_count: Arc<AtomicUsize>,
    start_requests: Arc<Mutex<Vec<ProvisionerWorkerStartRequest>>>,
    status: Arc<Mutex<ProvisionerWorkerStatus>>,
    status_error: Option<ProvisionerWorkerError>,
}

impl FakeWorker {
    fn idle() -> Self {
        Self::with_status(ProvisionerWorkerJobStatus::Idle)
    }

    fn succeeded() -> Self {
        Self::with_status(ProvisionerWorkerJobStatus::Succeeded)
    }

    fn with_status(status: ProvisionerWorkerJobStatus) -> Self {
        Self {
            start_count: Arc::default(),
            start_requests: Arc::default(),
            status: Arc::new(Mutex::new(ProvisionerWorkerStatus {
                phase: match status {
                    ProvisionerWorkerJobStatus::Succeeded => {
                        crate::provisioner_worker::ProvisionerWorkerPhase::Completed
                    }
                    _ => crate::provisioner_worker::ProvisionerWorkerPhase::Idle,
                },
                status,
                progress_percent: None,
                diagnostic: None,
            })),
            status_error: None,
        }
    }

    fn with_status_error(error: ProvisionerWorkerError) -> Self {
        Self {
            status_error: Some(error),
            ..Self::idle()
        }
    }
}

impl ProvisionerWorkerGateway for FakeWorker {
    fn start<'a>(
        &'a self,
        _provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
        request: &'a ProvisionerWorkerStartRequest,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            self.start_count.fetch_add(1, Ordering::SeqCst);
            self.start_requests
                .lock()
                .expect("start requests")
                .push(request.clone());
            Ok(ProvisionerWorkerStatus {
                status: ProvisionerWorkerJobStatus::Running,
                phase: crate::provisioner_worker::ProvisionerWorkerPhase::ValidatingRuntime,
                progress_percent: Some(25),
                diagnostic: None,
            })
        })
    }

    fn status<'a>(
        &'a self,
        _provisioner_status_url: &'a str,
        _token: &'a ProvisionerWorkerBearerToken,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisionerWorkerStatus, ProvisionerWorkerError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            if let Some(error) = &self.status_error {
                return Err(error.clone());
            }
            Ok(self.status.lock().expect("worker status").clone())
        })
    }
}

#[tokio::test]
async fn initiate_transitions_draft_to_provisioning() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service(catalog.clone(), FakeProvider::default());

    let result = service.initiate(&workspace.id).await.expect("initiate");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert_eq!(
        catalog
            .find_workspace_by_id(&workspace.id)
            .await
            .expect("find")
            .expect("workspace")
            .lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
}

#[tokio::test]
async fn sync_routes_runpod_workspace_through_provider_specific_steps() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.gpu_cloud_provider_id,
        GpuCloudProviderId::Runpod
    );
    assert_eq!(provider.discover_volumes_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);
    let create_volume_inputs = provider
        .create_volume_inputs
        .lock()
        .expect("create volume inputs");
    assert_eq!(create_volume_inputs.len(), 1);
    assert_eq!(
        create_volume_inputs[0].gpu_cloud_provider_id,
        workspace.gpu_cloud_provider_id
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
}

#[tokio::test]
async fn sync_creates_network_volume_once() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);

    service.sync(&workspace.id).await.expect("second sync");
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_refreshes_existing_volume_snapshot() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Creating,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result
            .workspace
            .persistent_storage_volume_snapshot
            .expect("volume")
            .provider_resource_status,
        ProviderResourceStatus::Ready
    );
    assert_eq!(provider.get_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_failed_when_volume_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Creating,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_volume_status: Some(ProviderResourceStatus::Failed),
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProviderResourceFailed
    );
    assert_eq!(failure.phase, WorkspaceProvisioningPhase::CreatingVolume);
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProviderResource
    );
    assert_eq!(
        failure.recovery_action,
        WorkspaceProvisioningRecoveryAction::CleanupWorkspaceResources
    );
    assert_eq!(provider.get_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn indeterminate_volume_create_marks_failed_without_losing_workspace() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_volume_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let service = service(catalog, provider);

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate
    );
}

#[tokio::test]
async fn provider_command_failure_preserves_workspace_metadata() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_volume_error: Some(WorkspaceResourceError::ProviderRateLimited),
        ..Default::default()
    };
    let service = service(catalog.clone(), provider);

    let error = service
        .sync(&workspace.id)
        .await
        .expect_err("rate limiting should be a command error");

    assert_eq!(error, WorkspaceProvisioningError::ProviderRateLimited);
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.persistent_storage_volume_snapshot.is_none());
    assert!(stored.last_provisioning_failure.is_none());
}

#[tokio::test]
async fn sync_creates_provisioning_pod_and_stores_worker_token() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        mount_path: "/workspace".to_string(),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker_tokens = Arc::new(Mutex::new(HashMap::new()));
    register_fake_provider(&catalog, &provider);
    let secrets = MemorySecretStore {
        api_key: Some(provider.api_key.clone()),
        worker_tokens: worker_tokens.clone(),
    };
    let resources = WorkspaceResourceService::new(secrets.clone(), catalog.clone());
    let service = WorkspaceProvisioningService::new(
        secrets,
        resources,
        catalog.clone(),
        FakeWorker::idle(),
        WorkspaceProvisioningCoordinator::default(),
        test_config(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
    let create_pod_inputs = provider
        .create_pod_inputs
        .lock()
        .expect("create pod inputs");
    assert_eq!(create_pod_inputs.len(), 1);
    assert_eq!(
        create_pod_inputs[0].provisioner_worker_image_ref,
        workspace.resolved_runtime_image.provisioner_image_ref
    );
    assert_eq!(worker_tokens.lock().expect("tokens").len(), 1);
    drop(create_pod_inputs);

    service.sync(&workspace.id).await.expect("second sync");
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_fails_when_same_name_provisioning_pod_exists_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_pods: vec![discovered_pod("pod-existing")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
    assert!(result
        .workspace
        .last_provisioning_failure
        .as_ref()
        .and_then(|failure| failure.diagnostic.as_ref())
        .is_some_and(|diagnostic| diagnostic.contains("pod-existing")));
}

#[tokio::test]
async fn indeterminate_pod_create_recovery_fails_with_orphaned_resource() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_pod_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        discovered_pods: Vec::new(),
        subsequent_discovered_pods: Some(vec![discovered_pod("pod-existing")]),
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 2);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_fails_when_multiple_discovered_provisioning_pods_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_pods: vec![discovered_pod("pod-1"), discovered_pod("pod-2")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources
    );
    assert_eq!(
        failure.phase,
        WorkspaceProvisioningPhase::StartingProvisioningPod
    );
    assert_eq!(provider.discover_pods_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn concurrent_sync_is_read_only() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let coordinator = WorkspaceProvisioningCoordinator::default();
    let _guard = coordinator.try_enter(&workspace.id).expect("enter");
    register_fake_provider(&catalog, &provider);
    let secrets = MemorySecretStore {
        api_key: Some(provider.api_key.clone()),
        worker_tokens: Arc::default(),
    };
    let resources = WorkspaceResourceService::new(secrets.clone(), catalog.clone());
    let service = WorkspaceProvisioningService::new(
        secrets,
        resources,
        catalog,
        FakeWorker::idle(),
        coordinator,
        test_config(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_starts_idle_worker_and_returns_worker_progress() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens);

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.get_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(worker.start_count.load(Ordering::SeqCst), 1);
    assert_eq!(
        result.progress.phase,
        WorkspaceProvisioningPhase::PreparingEnvironment
    );
    assert_eq!(result.progress.percent, Some(25));
    assert!(result.workspace.environment_prepared_at.is_none());
}

#[tokio::test]
async fn sync_starts_idle_worker_with_job_id() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let worker = FakeWorker::idle();
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        worker.clone(),
        worker_token_map(&workspace.id),
    );

    service.sync(&workspace.id).await.expect("sync");

    let requests = worker.start_requests.lock().expect("start requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].job_id, workspace.id);
    assert_eq!(
        requests[0].workflow_preset.id,
        workspace.placement_plan.selected_workflow_preset().id
    );
}

#[tokio::test]
async fn sync_treats_temporarily_unavailable_worker_as_running_progress() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::with_status_error(ProvisionerWorkerError::Unreachable);
    let service = service_with_parts(
        catalog.clone(),
        provider.clone(),
        worker.clone(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert_eq!(
        result.progress.phase,
        WorkspaceProvisioningPhase::PreparingEnvironment
    );
    assert_eq!(worker.start_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.create_pod_count.load(Ordering::SeqCst), 0);
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.last_provisioning_failure.is_none());
}

#[tokio::test]
async fn sync_preserves_existing_provisioner_status_url_when_provider_omits_it() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    let mut active_pod = active_pod();
    active_pod.provider_resource_status = ProviderResourceStatus::Creating;
    workspace.active_provisioning_pod_snapshot = Some(active_pod.clone());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_pod_status_url: Some(None),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider,
        FakeWorker::idle(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result
            .workspace
            .active_provisioning_pod_snapshot
            .expect("active pod")
            .provisioner_status_url,
        active_pod.provisioner_status_url
    );
}

#[tokio::test]
async fn worker_response_invalid_persists_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::with_status_error(ProvisionerWorkerError::InvalidPayload {
            diagnostic: Some(
                "code: invalid_request\nreason_code: missing_job_id\nmessage: job_id is required"
                    .to_string(),
            ),
        }),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerResponseInvalid
    );
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProvisionerWorker
    );
    assert_eq!(
        failure.diagnostic,
        Some(
            "code: invalid_request\nreason_code: missing_job_id\nmessage: job_id is required"
                .to_string()
        )
    );
}

#[tokio::test]
async fn worker_terminal_failure_persists_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::with_status_error(ProvisionerWorkerError::TerminalFailure {
            diagnostic: Some("safe diagnostic".to_string()),
        }),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    let failure = result
        .workspace
        .last_provisioning_failure
        .expect("failure should be persisted");
    assert_eq!(
        failure.code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerFailed
    );
    assert_eq!(
        failure.source,
        WorkspaceProvisioningFailureSource::ProvisionerWorker
    );
    assert_eq!(failure.diagnostic.as_deref(), Some("safe diagnostic"));
}

#[tokio::test]
async fn missing_worker_token_marks_failed_with_structured_failure_detail() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .progress
            .failure
            .expect("progress should expose failure")
            .code,
        WorkspaceProvisioningFailureCode::ProvisionerWorkerTokenMissing
    );
}

#[tokio::test]
async fn sync_persists_environment_timestamp_when_worker_succeeds() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::succeeded(),
        worker_token_map(&workspace.id),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result.workspace.environment_prepared_at.is_some());
    assert!(result.workspace.active_provisioning_pod_snapshot.is_some());
}

#[tokio::test]
async fn sync_terminates_pod_and_deletes_token_after_environment_is_prepared() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        tokens.clone(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert!(result.workspace.active_provisioning_pod_snapshot.is_none());
    assert!(result.workspace.last_provisioning_pod_snapshot.is_some());
    assert!(tokens.lock().expect("tokens").is_empty());
}

#[tokio::test]
async fn sync_creates_endpoint_template_after_environment_preparation() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 0);
    assert!(runpod_template_snapshot(&result.workspace).is_some());
    assert!(result.workspace.serverless_endpoint_snapshot.is_none());
}

#[tokio::test]
async fn sync_creates_endpoint_from_ready_template_and_keep_alive_plan() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 1);
    assert!(result.workspace.serverless_endpoint_snapshot.is_some());
}

#[tokio::test]
async fn sync_marks_failed_when_template_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Creating)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_template_status: Some(ProviderResourceStatus::Terminated),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderResourceTerminated
    );
    assert_eq!(provider.get_template_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_failed_when_endpoint_refresh_is_terminal() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
        provider_resource_status: ProviderResourceStatus::Creating,
        ..endpoint_snapshot()
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        get_endpoint_status: Some(ProviderResourceStatus::Unknown),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::ProviderResourceUnknown
    );
    assert_eq!(provider.get_endpoint_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_marks_workspace_ready_after_required_snapshots_are_ready() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let service = service_with_parts(
        catalog,
        FakeProvider::default(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Ready
    );
}

#[tokio::test]
async fn cancel_cleans_known_resources_and_returns_workspace_to_draft() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens.clone());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert!(result.workspace.provider_provisioning_snapshot.is_none());
}

#[tokio::test]
async fn cancel_skips_worker_cancel_when_provider_cleanup_succeeds() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let worker = FakeWorker::idle();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(catalog, provider.clone(), worker.clone(), tokens.clone());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert!(result.workspace.last_provisioning_failure.is_none());
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert!(result.workspace.provider_provisioning_snapshot.is_none());
}

#[tokio::test]
async fn cancel_marks_failed_and_preserves_metadata_when_cleanup_fails() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        delete_endpoint_error: Some(WorkspaceResourceError::ProviderApiUnavailable),
        ..Default::default()
    };
    let service = service_with_parts(catalog, provider, FakeWorker::idle(), Arc::default());

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Failed
    );
    assert_eq!(
        result
            .workspace
            .last_provisioning_failure
            .expect("failure should be persisted")
            .code,
        WorkspaceProvisioningFailureCode::CancellationCleanupFailed
    );
    assert!(result.workspace.serverless_endpoint_snapshot.is_some());
    assert!(result.workspace.provider_provisioning_snapshot.is_some());
}

#[tokio::test]
async fn sync_fails_when_same_name_network_volume_exists_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_volumes: vec![discovered_volume("volume-existing")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert_eq!(provider.discover_volumes_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
    assert!(result
        .workspace
        .last_provisioning_failure
        .as_ref()
        .and_then(|failure| failure.diagnostic.as_ref())
        .is_some_and(|diagnostic| diagnostic.contains("volume-existing")));
}

#[tokio::test]
async fn sync_fails_when_multiple_discovered_network_volumes_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_volumes: vec![discovered_volume("volume-1"), discovered_volume("volume-2")],
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_when_same_name_endpoint_template_exists_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_templates: vec![discovered_template("template-existing")],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert!(runpod_template_snapshot(&result.workspace).is_none());
    assert_eq!(provider.discover_templates_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_when_multiple_discovered_endpoint_templates_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_templates: vec![
            discovered_template("template-1"),
            discovered_template("template-2"),
        ],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_reuses_ready_endpoint_template_and_creates_endpoint() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: "template-old".to_string(),
            endpoint_worker_image_ref: sample_endpoint_worker_image_ref(),
            mount_path: "/workspace".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
        }),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(runpod_template_snapshot(&result.workspace).is_some());
    assert_eq!(provider.get_template_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sync_refreshes_mismatched_ready_template_before_replacing_it() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: "template-old".to_string(),
            endpoint_worker_image_ref: "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:3333333333333333333333333333333333333333333333333333333333333333".to_string(),
            mount_path: "/workspace".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
        }),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert!(result.workspace.serverless_endpoint_snapshot.is_some());
    assert!(runpod_template_snapshot(&result.workspace).is_some());
    assert_eq!(provider.get_template_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_when_same_name_serverless_endpoint_exists_before_create() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_endpoints: vec![discovered_endpoint("endpoint-existing")],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert!(result.workspace.serverless_endpoint_snapshot.is_none());
    assert_eq!(provider.discover_endpoints_count.load(Ordering::SeqCst), 1);
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sync_fails_when_multiple_discovered_serverless_endpoints_exist() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        discovered_endpoints: vec![
            discovered_endpoint("endpoint-1"),
            discovered_endpoint("endpoint-2"),
        ],
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn indeterminate_volume_create_recovery_fails_with_orphaned_resource() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_volume_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        subsequent_discovered_volumes: Some(vec![discovered_volume("volume-existing")]),
        ..Default::default()
    };
    let service = service(catalog, provider.clone());

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert!(result
        .workspace
        .persistent_storage_volume_snapshot
        .is_none());
    assert_eq!(provider.discover_volumes_count.load(Ordering::SeqCst), 2);
    assert_eq!(provider.create_volume_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn indeterminate_template_create_recovery_fails_with_orphaned_resource() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_template_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        subsequent_discovered_templates: Some(vec![discovered_template("template-existing")]),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert!(runpod_template_snapshot(&result.workspace).is_none());
    assert_eq!(provider.discover_templates_count.load(Ordering::SeqCst), 2);
    assert_eq!(provider.create_template_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn indeterminate_endpoint_create_recovery_fails_with_orphaned_resource() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider {
        create_endpoint_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        subsequent_discovered_endpoints: Some(vec![discovered_endpoint("endpoint-existing")]),
        ..Default::default()
    };
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );

    let result = service.sync(&workspace.id).await.expect("sync");

    assert_failure(
        &result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOrphanedResources,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert!(result.workspace.serverless_endpoint_snapshot.is_none());
    assert_eq!(provider.discover_endpoints_count.load(Ordering::SeqCst), 2);
    assert_eq!(provider.create_endpoint_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn indeterminate_create_failures_do_not_retry_create_on_next_sync() {
    let volume_catalog = MemoryWorkspaceCatalog::default();
    let mut volume_workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    volume_workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    volume_catalog
        .insert_workspace(&volume_workspace)
        .await
        .expect("insert");
    let volume_provider = FakeProvider {
        create_volume_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let volume_service = service(volume_catalog, volume_provider.clone());
    let volume_result = volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &volume_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        volume_provider.create_volume_count.load(Ordering::SeqCst),
        1
    );

    let pod_catalog = MemoryWorkspaceCatalog::default();
    let pod_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000002");
    pod_catalog
        .insert_workspace(&pod_workspace)
        .await
        .expect("insert");
    let pod_provider = FakeProvider {
        create_pod_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let pod_service = service(pod_catalog, pod_provider.clone());
    let pod_result = pod_service.sync(&pod_workspace.id).await.expect("sync");
    assert_failure(
        &pod_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    pod_service
        .sync(&pod_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(pod_provider.create_pod_count.load(Ordering::SeqCst), 1);

    let template_catalog = MemoryWorkspaceCatalog::default();
    let mut template_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000003");
    template_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    template_catalog
        .insert_workspace(&template_workspace)
        .await
        .expect("insert");
    let template_provider = FakeProvider {
        create_template_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let template_service = service_with_parts(
        template_catalog,
        template_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let template_result = template_service
        .sync(&template_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &template_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    template_service
        .sync(&template_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        template_provider
            .create_template_count
            .load(Ordering::SeqCst),
        1
    );

    let endpoint_catalog = MemoryWorkspaceCatalog::default();
    let mut endpoint_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000004");
    endpoint_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    endpoint_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
        });
    endpoint_catalog
        .insert_workspace(&endpoint_workspace)
        .await
        .expect("insert");
    let endpoint_provider = FakeProvider {
        create_endpoint_error: Some(WorkspaceResourceError::ProviderOperationIndeterminate),
        ..Default::default()
    };
    let endpoint_service = service_with_parts(
        endpoint_catalog,
        endpoint_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let endpoint_result = endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &endpoint_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderOperationIndeterminate,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("second sync");
    assert_eq!(
        endpoint_provider
            .create_endpoint_count
            .load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn missing_tracked_resources_mark_workspace_failed_without_recreate() {
    let volume_catalog = MemoryWorkspaceCatalog::default();
    let mut volume_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    volume_workspace
        .persistent_storage_volume_snapshot
        .as_mut()
        .expect("volume")
        .provider_resource_status = ProviderResourceStatus::Creating;
    volume_catalog
        .insert_workspace(&volume_workspace)
        .await
        .expect("insert");
    let volume_provider = FakeProvider {
        get_volume_error: Some(WorkspaceResourceError::ProviderResourceNotFound),
        ..Default::default()
    };
    let volume_service = service(volume_catalog, volume_provider.clone());
    let volume_result = volume_service
        .sync(&volume_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &volume_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingVolume,
    );
    assert!(volume_result
        .workspace
        .persistent_storage_volume_snapshot
        .is_some());
    assert_eq!(
        volume_provider.create_volume_count.load(Ordering::SeqCst),
        0
    );

    let pod_catalog = MemoryWorkspaceCatalog::default();
    let mut pod_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000002");
    pod_workspace.active_provisioning_pod_snapshot = Some(active_pod());
    pod_catalog
        .insert_workspace(&pod_workspace)
        .await
        .expect("insert");
    let pod_provider = FakeProvider {
        get_pod_error: Some(WorkspaceResourceError::ProviderResourceNotFound),
        ..Default::default()
    };
    let pod_service = service(pod_catalog, pod_provider.clone());
    let pod_result = pod_service.sync(&pod_workspace.id).await.expect("sync");
    assert_failure(
        &pod_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::StartingProvisioningPod,
    );
    assert!(pod_result
        .workspace
        .active_provisioning_pod_snapshot
        .is_some());
    assert_eq!(pod_provider.create_pod_count.load(Ordering::SeqCst), 0);

    let template_catalog = MemoryWorkspaceCatalog::default();
    let mut template_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000003");
    template_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    template_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Creating)),
        });
    template_catalog
        .insert_workspace(&template_workspace)
        .await
        .expect("insert");
    let template_provider = FakeProvider {
        get_template_error: Some(WorkspaceResourceError::ProviderResourceNotFound),
        ..Default::default()
    };
    let template_service = service_with_parts(
        template_catalog,
        template_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let template_result = template_service
        .sync(&template_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &template_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingEndpointTemplate,
    );
    assert!(runpod_template_snapshot(&template_result.workspace).is_some());
    assert_eq!(
        template_provider
            .create_template_count
            .load(Ordering::SeqCst),
        0
    );

    let endpoint_catalog = MemoryWorkspaceCatalog::default();
    let mut endpoint_workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000004");
    endpoint_workspace.environment_prepared_at = Some("2026-05-08T00:00:00Z".to_string());
    endpoint_workspace.provider_provisioning_snapshot =
        Some(ProviderProvisioningSnapshot::Runpod {
            endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
        });
    endpoint_workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
        provider_resource_status: ProviderResourceStatus::Creating,
        ..endpoint_snapshot()
    });
    endpoint_catalog
        .insert_workspace(&endpoint_workspace)
        .await
        .expect("insert");
    let endpoint_provider = FakeProvider {
        get_endpoint_error: Some(WorkspaceResourceError::ProviderResourceNotFound),
        ..Default::default()
    };
    let endpoint_service = service_with_parts(
        endpoint_catalog,
        endpoint_provider.clone(),
        FakeWorker::idle(),
        Arc::default(),
    );
    let endpoint_result = endpoint_service
        .sync(&endpoint_workspace.id)
        .await
        .expect("sync");
    assert_failure(
        &endpoint_result.workspace,
        WorkspaceProvisioningFailureCode::ProviderResourceMissing,
        WorkspaceProvisioningPhase::CreatingEndpoint,
    );
    assert!(endpoint_result
        .workspace
        .serverless_endpoint_snapshot
        .is_some());
    assert_eq!(
        endpoint_provider
            .create_endpoint_count
            .load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn cancel_deletes_worker_token_even_without_active_pod_snapshot() {
    let catalog = MemoryWorkspaceCatalog::default();
    let workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let service = service_with_parts(
        catalog,
        provider.clone(),
        FakeWorker::idle(),
        tokens.clone(),
    );

    let result = service.cancel(&workspace.id).await.expect("cancel");

    assert_eq!(
        result.workspace.lifecycle_state,
        WorkspaceLifecycleState::Draft
    );
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 1);
    assert!(tokens.lock().expect("tokens").is_empty());
}

#[tokio::test]
async fn cancel_conflict_returns_error_without_cleanup_side_effects() {
    let catalog = MemoryWorkspaceCatalog::default();
    let mut workspace =
        provisioning_workspace_with_ready_volume("018f6a40-0000-7000-8000-000000000001");
    workspace.active_provisioning_pod_snapshot = Some(active_pod());
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(template_snapshot(ProviderResourceStatus::Ready)),
    });
    workspace.serverless_endpoint_snapshot = Some(endpoint_snapshot());
    catalog.insert_workspace(&workspace).await.expect("insert");
    let provider = FakeProvider::default();
    let tokens = worker_token_map(&workspace.id);
    let coordinator = WorkspaceProvisioningCoordinator::default();
    let _guard = coordinator.try_enter(&workspace.id).expect("enter");
    register_fake_provider(&catalog, &provider);
    let secrets = MemorySecretStore {
        api_key: Some(provider.api_key.clone()),
        worker_tokens: tokens.clone(),
    };
    let resources = WorkspaceResourceService::new(secrets.clone(), catalog.clone());
    let service = WorkspaceProvisioningService::new(
        secrets,
        resources,
        catalog.clone(),
        FakeWorker::idle(),
        coordinator,
        test_config(),
    );

    let error = service
        .cancel(&workspace.id)
        .await
        .expect_err("cancel should conflict");

    assert_eq!(error, WorkspaceProvisioningError::ProviderOperationConflict);
    assert_eq!(provider.delete_endpoint_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_template_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_pod_count.load(Ordering::SeqCst), 0);
    assert_eq!(provider.delete_volume_count.load(Ordering::SeqCst), 0);
    assert!(tokens.lock().expect("tokens").contains_key(&workspace.id));
    let stored = catalog
        .find_workspace_by_id(&workspace.id)
        .await
        .expect("find")
        .expect("workspace");
    assert_eq!(
        stored.lifecycle_state,
        WorkspaceLifecycleState::Provisioning
    );
    assert!(stored.active_provisioning_pod_snapshot.is_some());
    assert!(stored.serverless_endpoint_snapshot.is_some());
}

fn service(
    catalog: MemoryWorkspaceCatalog,
    provider: FakeProvider,
) -> TestWorkspaceProvisioningService {
    service_with_parts(catalog, provider, FakeWorker::idle(), Arc::default())
}

fn service_with_parts(
    catalog: MemoryWorkspaceCatalog,
    provider: FakeProvider,
    worker: FakeWorker,
    worker_tokens: Arc<Mutex<HashMap<String, String>>>,
) -> TestWorkspaceProvisioningService {
    register_fake_provider(&catalog, &provider);
    let secrets = MemorySecretStore {
        api_key: Some(provider.api_key.clone()),
        worker_tokens,
    };
    let resources = WorkspaceResourceService::new(secrets.clone(), catalog.clone());
    WorkspaceProvisioningService::new(
        secrets,
        resources,
        catalog,
        worker,
        WorkspaceProvisioningCoordinator::default(),
        test_config(),
    )
}

fn provisioning_workspace_with_ready_volume(id: &str) -> Workspace {
    let mut workspace = sample_workspace(id);
    workspace.lifecycle_state = WorkspaceLifecycleState::Provisioning;
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        mount_path: "/workspace".to_string(),
    });
    workspace
}

fn active_pod() -> ProvisioningPodSnapshot {
    ProvisioningPodSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "pod-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Running,
        provisioner_status_url: "http://203.0.113.10:30001/status".to_string(),
    }
}

fn discovered_pod(provider_resource_id: &str) -> ProvisioningPodObservation {
    ProvisioningPodObservation {
        provider_resource_id: provider_resource_id.to_string(),
        provider_resource_status: ProviderResourceStatus::Running,
        provisioner_status_url: Some(format!(
            "https://{provider_resource_id}-8080.proxy.runpod.net/status"
        )),
    }
}

fn discovered_volume(provider_resource_id: &str) -> NetworkVolumeObservation {
    NetworkVolumeObservation {
        provider_resource_id: provider_resource_id.to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        mount_path: "/workspace".to_string(),
    }
}

fn discovered_template(template_id: &str) -> EndpointTemplateObservation {
    EndpointTemplateObservation {
        template_id: template_id.to_string(),
        endpoint_worker_image_ref: sample_endpoint_worker_image_ref(),
        mount_path: "/workspace".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
    }
}

fn template_snapshot(status: ProviderResourceStatus) -> RunPodEndpointTemplateSnapshot {
    RunPodEndpointTemplateSnapshot {
        template_id: "template-1".to_string(),
        endpoint_worker_image_ref: sample_endpoint_worker_image_ref(),
        mount_path: "/workspace".to_string(),
        provider_resource_status: status,
    }
}

fn sample_endpoint_worker_image_ref() -> String {
    sample_runtime_snapshot().endpoint_image_ref
}

fn discovered_endpoint(provider_resource_id: &str) -> ServerlessEndpointObservation {
    ServerlessEndpointObservation {
        provider_resource_id: provider_resource_id.to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        endpoint_invoke_url: format!("https://api.runpod.ai/v2/{provider_resource_id}/runsync"),
    }
}

fn endpoint_snapshot() -> ServerlessEndpointSnapshot {
    ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        endpoint_invoke_url: "https://api.runpod.ai/v2/endpoint-1/runsync".to_string(),
    }
}

fn assert_failure(
    workspace: &Workspace,
    code: WorkspaceProvisioningFailureCode,
    phase: WorkspaceProvisioningPhase,
) {
    assert_eq!(workspace.lifecycle_state, WorkspaceLifecycleState::Failed);
    let failure = workspace
        .last_provisioning_failure
        .as_ref()
        .expect("failure should be persisted");
    assert_eq!(failure.code, code);
    assert_eq!(failure.phase, phase);
}

fn worker_token_map(workspace_id: &str) -> Arc<Mutex<HashMap<String, String>>> {
    Arc::new(Mutex::new(HashMap::from([(
        workspace_id.to_string(),
        "worker-token".to_string(),
    )])))
}

fn test_config() -> WorkspaceProvisioningConfig {
    WorkspaceProvisioningConfig {
        volume_mount_path: "/workspace".to_string(),
    }
}
