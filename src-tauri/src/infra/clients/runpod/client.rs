use std::collections::HashMap;

use graphql_client::GraphQLQuery;
use hmac::{Hmac, Mac};
use reqwest::header::CONTENT_TYPE;
use secrecy::ExposeSecret;
use sha2::Sha256;

use crate::infra::clients::{graphql::GraphqlResponseExt, http, http::ResponseExt, NetworkError};

use super::{
    generated::{
        Endpoint, EndpointCreateInput, EndpointCreateInputComputeType,
        EndpointCreateInputDataCenterIdsItem, EndpointCreateInputGpuTypeIdsItem, NetworkVolume,
        NetworkVolumeCreateInput, Pod, PodCreateInput, PodCreateInputComputeType,
        PodCreateInputDataCenterIdsItem, Template, TemplateCreateInput,
    },
    queries::{myself, placement, Myself, Placement},
    CreateEndpointRequest, CreateEndpointResponse, CreateNetworkVolumeRequest,
    CreateNetworkVolumeResponse, CreatePodRequest, CreatePodResponse, CreateTemplateRequest,
    CreateTemplateResponse, DeleteEndpointRequest, DeleteNetworkVolumeRequest, DeletePodRequest,
    DeleteTemplateRequest, IdentityRequest, IdentityResponse, PlacementDatacenter,
    PlacementGpuAvailability, PlacementGpuType, PlacementRequest, PlacementResponse,
    ProvisionerStatusRequest, ProvisionerStatusResponse,
};

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const RESOURCE_PREFIX: &str = "luma-forge";
const PROVISIONER_PORT: &str = "8000/http";

#[derive(Clone)]
pub struct RunpodClient {
    http: reqwest::Client,
}

impl RunpodClient {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            http: http::client()?,
        })
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn identity(
        &self,
        #[diagnostic(show)] request: IdentityRequest,
    ) -> Result<IdentityResponse, NetworkError> {
        let response: myself::ResponseData = self
            .http
            .post(GRAPHQL_URL)
            .bearer_auth(request.credential.expose_secret())
            .header(CONTENT_TYPE, "application/json")
            .json(&Myself::build_query(myself::Variables {}))
            .send()
            .await
            .into_graphql_data()
            .await?;
        identity_response(response)
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn placement(
        &self,
        #[diagnostic(show)] request: PlacementRequest,
    ) -> Result<PlacementResponse, NetworkError> {
        let response: placement::ResponseData = self
            .http
            .post(GRAPHQL_URL)
            .bearer_auth(request.credential.expose_secret())
            .header(CONTENT_TYPE, "application/json")
            .json(&Placement::build_query(placement::Variables {}))
            .send()
            .await
            .into_graphql_data()
            .await?;
        placement_response(response)
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn create_network_volume(
        &self,
        #[diagnostic(show)] request: CreateNetworkVolumeRequest,
    ) -> Result<CreateNetworkVolumeResponse, NetworkError> {
        let response = self
            .http
            .post(format!("{REST_BASE_URL}/networkvolumes"))
            .bearer_auth(request.credential.expose_secret())
            .json(&network_volume_create_input(&request))
            .send()
            .await
            .into_json::<NetworkVolume>()
            .await?;
        Ok(CreateNetworkVolumeResponse { id: response.id })
    }

    #[crate::diagnostics::diagnostic]
    pub async fn delete_network_volume(
        &self,
        #[diagnostic(show)] request: DeleteNetworkVolumeRequest,
    ) -> Result<(), NetworkError> {
        self.delete("networkvolumes", request.credential, request.id)
            .await
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn create_pod(
        &self,
        #[diagnostic(show)] request: CreatePodRequest,
    ) -> Result<CreatePodResponse, NetworkError> {
        let input = pod_create_input(&request)?;
        let response = self
            .http
            .post(format!("{REST_BASE_URL}/pods"))
            .bearer_auth(request.credential.expose_secret())
            .json(&input)
            .send()
            .await
            .into_json::<Pod>()
            .await?;
        Ok(CreatePodResponse { id: response.id })
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn provisioner_status(
        &self,
        #[diagnostic(show)] request: ProvisionerStatusRequest,
    ) -> Result<ProvisionerStatusResponse, NetworkError> {
        let bearer_token = derive_bearer_token(&request.credential, &request.workspace_id)?;
        self.http
            .get(format!(
                "https://{}-8000.proxy.runpod.net/status",
                request.pod_id
            ))
            .bearer_auth(bearer_token)
            .send()
            .await
            .into_json()
            .await
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn create_endpoint(
        &self,
        #[diagnostic(show)] request: CreateEndpointRequest,
    ) -> Result<CreateEndpointResponse, NetworkError> {
        let input = endpoint_create_input(&request)?;
        let response = self
            .http
            .post(format!("{REST_BASE_URL}/endpoints"))
            .bearer_auth(request.credential.expose_secret())
            .json(&input)
            .send()
            .await
            .into_json::<Endpoint>()
            .await?;
        Ok(CreateEndpointResponse { id: response.id })
    }

    #[crate::diagnostics::diagnostic(show_output)]
    pub async fn create_template(
        &self,
        #[diagnostic(show)] request: CreateTemplateRequest,
    ) -> Result<CreateTemplateResponse, NetworkError> {
        let response = self
            .http
            .post(format!("{REST_BASE_URL}/templates"))
            .bearer_auth(request.credential.expose_secret())
            .json(&template_create_input(&request))
            .send()
            .await
            .into_json::<Template>()
            .await?;
        Ok(CreateTemplateResponse { id: response.id })
    }

    #[crate::diagnostics::diagnostic]
    pub async fn delete_pod(
        &self,
        #[diagnostic(show)] request: DeletePodRequest,
    ) -> Result<(), NetworkError> {
        self.delete("pods", request.credential, request.id).await
    }

    #[crate::diagnostics::diagnostic]
    pub async fn delete_template(
        &self,
        #[diagnostic(show)] request: DeleteTemplateRequest,
    ) -> Result<(), NetworkError> {
        self.delete("templates", request.credential, request.id)
            .await
    }

    #[crate::diagnostics::diagnostic]
    pub async fn delete_endpoint(
        &self,
        #[diagnostic(show)] request: DeleteEndpointRequest,
    ) -> Result<(), NetworkError> {
        self.delete("endpoints", request.credential, request.id)
            .await
    }

    async fn delete(
        &self,
        resource: &str,
        credential: secrecy::SecretString,
        id: String,
    ) -> Result<(), NetworkError> {
        self.http
            .delete(format!("{REST_BASE_URL}/{resource}/{id}"))
            .bearer_auth(credential.expose_secret())
            .send()
            .await
            .into_response()?;
        Ok(())
    }
}

fn identity_response(response: myself::ResponseData) -> Result<IdentityResponse, NetworkError> {
    let identity = response.myself.ok_or(NetworkError::InvalidResponse)?;
    Ok(IdentityResponse {
        user_id: identity.id,
        email: identity.email,
    })
}

fn placement_response(
    response: placement::ResponseData,
) -> Result<PlacementResponse, NetworkError> {
    let identity = response.myself.ok_or(NetworkError::InvalidResponse)?;
    let gpu_types = response.gpu_types.map(|gpu_types| {
        gpu_types
            .into_iter()
            .map(|gpu| {
                gpu.map(|gpu| PlacementGpuType {
                    id: gpu.id,
                    display_name: gpu.display_name,
                    memory_gb: gpu.memory_in_gb,
                })
            })
            .collect()
    });
    let datacenters = identity.datacenters.map(|datacenters| {
        datacenters
            .into_iter()
            .map(|datacenter| {
                datacenter.map(|datacenter| PlacementDatacenter {
                    id: datacenter.id,
                    name: datacenter.name,
                    gpu_availability: datacenter.gpu_availability.map(|availability| {
                        availability
                            .into_iter()
                            .map(|gpu| {
                                gpu.map(|gpu| PlacementGpuAvailability {
                                    gpu_type_id: gpu.gpu_type_id,
                                    available: gpu.available,
                                    stock_status: gpu.stock_status,
                                })
                            })
                            .collect()
                    }),
                })
            })
            .collect()
    });
    Ok(PlacementResponse {
        gpu_types,
        datacenters,
    })
}

fn network_volume_create_input(request: &CreateNetworkVolumeRequest) -> NetworkVolumeCreateInput {
    NetworkVolumeCreateInput {
        data_center_id: request.datacenter_id.clone(),
        name: resource_name(&request.workspace_id, "volume"),
        size: request.size_gb,
    }
}

fn pod_create_input(request: &CreatePodRequest) -> Result<PodCreateInput, NetworkError> {
    let mut env = HashMap::from([
        (
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_owned(),
            derive_bearer_token(&request.credential, &request.workspace_id)?,
        ),
        (
            "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS".to_owned(),
            request.required_model_assets.to_string(),
        ),
    ]);
    if let Some(credential) = &request.hugging_face_credential {
        env.insert(
            "LUMA_FORGE_HUGGING_FACE_API_KEY".to_owned(),
            credential.expose_secret().to_owned(),
        );
    }

    Ok(PodCreateInput {
        compute_type: Some(PodCreateInputComputeType::Cpu),
        data_center_ids: vec![request
            .datacenter_id
            .parse::<PodCreateInputDataCenterIdsItem>()
            .map_err(|_| NetworkError::InvalidResponse)?],
        env,
        image_name: Some(request.provisioner_image_ref.clone()),
        name: Some(resource_name(&request.workspace_id, "provisioner")),
        network_volume_id: Some(request.network_volume_id.clone()),
        ports: vec![PROVISIONER_PORT.to_owned()],
        ..Default::default()
    })
}

fn endpoint_create_input(
    request: &CreateEndpointRequest,
) -> Result<EndpointCreateInput, NetworkError> {
    Ok(EndpointCreateInput {
        allowed_cuda_versions: Vec::new(),
        compute_type: Some(EndpointCreateInputComputeType::Gpu),
        cpu_flavor_ids: Vec::new(),
        data_center_ids: vec![request
            .datacenter_id
            .parse::<EndpointCreateInputDataCenterIdsItem>()
            .map_err(|_| NetworkError::InvalidResponse)?],
        execution_timeout_ms: None,
        flashboot: None,
        gpu_count: None,
        gpu_type_ids: vec![request
            .gpu_id
            .parse::<EndpointCreateInputGpuTypeIdsItem>()
            .map_err(|_| NetworkError::InvalidResponse)?],
        idle_timeout: None,
        min_cuda_version: None,
        name: Some(resource_name(&request.workspace_id, "endpoint")),
        network_volume_id: Some(request.network_volume_id.clone()),
        network_volume_ids: Vec::new(),
        scaler_type: None,
        scaler_value: None,
        template_id: request.template_id.clone(),
        vcpu_count: None,
        workers_max: Some(request.workers_max),
        workers_min: Some(request.workers_min),
    })
}

fn template_create_input(request: &CreateTemplateRequest) -> TemplateCreateInput {
    TemplateCreateInput {
        category: None,
        container_disk_in_gb: None,
        container_registry_auth_id: None,
        docker_entrypoint: Vec::new(),
        docker_start_cmd: Vec::new(),
        env: HashMap::new(),
        image_name: request.image_ref.clone(),
        is_public: Some(false),
        is_serverless: Some(true),
        name: resource_name(&request.workspace_id, "template"),
        ports: Vec::new(),
        readme: None,
        volume_in_gb: None,
        volume_mount_path: None,
    }
}

fn derive_bearer_token(
    credential: &secrecy::SecretString,
    workspace_id: &str,
) -> Result<String, NetworkError> {
    let mut mac = Hmac::<Sha256>::new_from_slice(credential.expose_secret().as_bytes())
        .map_err(|_| NetworkError::RequestFailed)?;
    mac.update(workspace_id.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn resource_name(workspace_id: &str, resource: &str) -> String {
    format!("{RESOURCE_PREFIX}-{workspace_id}-{resource}")
}

#[cfg(test)]
mod tests {
    use secrecy::{ExposeSecret, SecretString};

    use super::{placement, placement_response, pod_create_input, CreatePodRequest, NetworkError};

    fn request() -> CreatePodRequest {
        CreatePodRequest {
            credential: SecretString::from("runpod-secret"),
            hugging_face_credential: Some(SecretString::from("hugging-face-secret")),
            workspace_id: "workspace-1".into(),
            datacenter_id: "EU-RO-1".into(),
            provisioner_image_ref: "registry/provisioner:latest".into(),
            network_volume_id: "volume-1".into(),
            required_model_assets: serde_json::json!([{"id": "model-1"}]),
        }
    }

    #[test]
    fn pod_request_debug_redacts_credentials_and_omits_model_assets() {
        let formatted = format!("{:?}", request());

        assert!(formatted.contains("workspace-1"));
        assert!(formatted.contains("EU-RO-1"));
        assert!(formatted.contains("credential: [REDACTED]"));
        assert!(formatted.contains("hugging_face_credential: [REDACTED]"));
        assert!(!formatted.contains("runpod-secret"));
        assert!(!formatted.contains("hugging-face-secret"));
        assert!(!formatted.contains("required_model_assets"));
        assert!(!formatted.contains("model-1"));
    }

    #[test]
    fn pod_wire_mapper_exposes_secrets_only_in_required_environment_entries() {
        let input = pod_create_input(&request()).unwrap();

        assert_eq!(
            input.env["LUMA_FORGE_HUGGING_FACE_API_KEY"],
            "hugging-face-secret"
        );
        assert_eq!(
            input.env["LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS"],
            "[{\"id\":\"model-1\"}]"
        );
        assert_ne!(
            input.env["LUMA_FORGE_PROVISIONER_BEARER_TOKEN"],
            request().credential.expose_secret()
        );
        assert_eq!(input.env.len(), 3);
    }

    #[test]
    fn placement_mapper_rejects_missing_identity() {
        let response: placement::ResponseData = serde_json::from_value(serde_json::json!({
            "gpuTypes": [],
            "myself": null
        }))
        .unwrap();

        assert_eq!(
            placement_response(response).unwrap_err(),
            NetworkError::InvalidResponse
        );
    }

    #[test]
    fn placement_mapper_preserves_nullable_lists_and_items() {
        let null_lists: placement::ResponseData = serde_json::from_value(serde_json::json!({
            "gpuTypes": null,
            "myself": { "datacenters": null }
        }))
        .unwrap();
        let null_items: placement::ResponseData = serde_json::from_value(serde_json::json!({
            "gpuTypes": [null],
            "myself": { "datacenters": [null] }
        }))
        .unwrap();

        let null_lists = placement_response(null_lists).unwrap();
        assert!(null_lists.gpu_types.is_none());
        assert!(null_lists.datacenters.is_none());

        let null_items = placement_response(null_items).unwrap();
        assert!(null_items.gpu_types.unwrap()[0].is_none());
        assert!(null_items.datacenters.unwrap()[0].is_none());
    }
}
