use std::collections::HashMap;

use graphql_client::GraphQLQuery;
use hmac::{Hmac, Mac};
use reqwest::header::CONTENT_TYPE;
use secrecy::ExposeSecret;
use sha2::Sha256;

use crate::providers::{graphql::GraphqlResponseExt, http, http::ResponseExt, NetworkError};

use super::{
    queries::{myself, placement, Myself, Placement},
    CreateEndpointRequest, CreateEndpointResponse, CreateNetworkVolumeRequest,
    CreateNetworkVolumeResponse, CreatePodRequest, CreatePodResponse, CreateTemplateRequest,
    CreateTemplateResponse, DeleteEndpointRequest, DeleteNetworkVolumeRequest, DeletePodRequest,
    DeleteTemplateRequest, EndpointSummary, IdentityRequest, IdentityResponse,
    ListEndpointsRequest, ListNetworkVolumesRequest, ListPodsRequest, ListTemplatesRequest,
    NetworkVolumeSummary, PlacementDatacenter, PlacementGpuAvailability, PlacementGpuType,
    PlacementRequest, PlacementResponse, PodSummary, ProvisionerStatusRequest,
    ProvisionerStatusResponse, TemplateSummary,
};

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const REST_BASE_URL: &str = "https://rest.runpod.io/v1";
const PROVISIONER_PORT: &str = "8000/http";

#[derive(Clone)]
pub struct RunpodProvider {
    http: reqwest::Client,
}

impl RunpodProvider {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            http: http::client()?,
        })
    }

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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
            .into_json::<CreatedResourceResponse>()
            .await?;
        Ok(CreateNetworkVolumeResponse { id: response.id })
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn list_network_volumes(
        &self,
        #[diagnostic(show)] request: ListNetworkVolumesRequest,
    ) -> Result<Vec<NetworkVolumeSummary>, NetworkError> {
        self.http
            .get(format!("{REST_BASE_URL}/networkvolumes"))
            .bearer_auth(request.credential.expose_secret())
            .send()
            .await
            .into_json()
            .await
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn delete_network_volume(
        &self,
        #[diagnostic(show)] request: DeleteNetworkVolumeRequest,
    ) -> Result<(), NetworkError> {
        self.delete("networkvolumes", request.credential, request.id)
            .await
    }

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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
            .into_json::<CreatedResourceResponse>()
            .await?;
        Ok(CreatePodResponse { id: response.id })
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn list_pods(
        &self,
        #[diagnostic(show)] request: ListPodsRequest,
    ) -> Result<Vec<PodSummary>, NetworkError> {
        self.http
            .get(format!("{REST_BASE_URL}/pods"))
            .bearer_auth(request.credential.expose_secret())
            .query(&[
                ("name", request.name.as_str()),
                ("includeNetworkVolume", "true"),
            ])
            .send()
            .await
            .into_json()
            .await
    }

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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

    #[luma_diagnostics::diagnostic(show_output, show_error)]
    pub async fn create_endpoint(
        &self,
        #[diagnostic(show)] request: CreateEndpointRequest,
    ) -> Result<CreateEndpointResponse, NetworkError> {
        let input = endpoint_create_input(&request);
        let response = self
            .http
            .post(format!("{REST_BASE_URL}/endpoints"))
            .bearer_auth(request.credential.expose_secret())
            .json(&input)
            .send()
            .await
            .into_json::<CreatedResourceResponse>()
            .await?;
        Ok(CreateEndpointResponse { id: response.id })
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn list_endpoints(
        &self,
        #[diagnostic(show)] request: ListEndpointsRequest,
    ) -> Result<Vec<EndpointSummary>, NetworkError> {
        self.http
            .get(format!("{REST_BASE_URL}/endpoints"))
            .bearer_auth(request.credential.expose_secret())
            .send()
            .await
            .into_json()
            .await
    }

    #[luma_diagnostics::diagnostic(show_output, show_error)]
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
            .into_json::<CreatedResourceResponse>()
            .await?;
        Ok(CreateTemplateResponse { id: response.id })
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn list_templates(
        &self,
        #[diagnostic(show)] request: ListTemplatesRequest,
    ) -> Result<Vec<TemplateSummary>, NetworkError> {
        self.http
            .get(format!("{REST_BASE_URL}/templates"))
            .bearer_auth(request.credential.expose_secret())
            .query(&[("includeEndpointBoundTemplates", "true")])
            .send()
            .await
            .into_json()
            .await
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn delete_pod(
        &self,
        #[diagnostic(show)] request: DeletePodRequest,
    ) -> Result<(), NetworkError> {
        self.delete("pods", request.credential, request.id).await
    }

    #[luma_diagnostics::diagnostic(show_error)]
    pub async fn delete_template(
        &self,
        #[diagnostic(show)] request: DeleteTemplateRequest,
    ) -> Result<(), NetworkError> {
        self.delete("templates", request.credential, request.id)
            .await
    }

    #[luma_diagnostics::diagnostic(show_error)]
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
    let datacenters = response.data_centers.map(|datacenters| {
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
        name: request.name.clone(),
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
        compute_type: "CPU",
        data_center_ids: vec![request.datacenter_id.clone()],
        env,
        image_name: request.provisioner_image_ref.clone(),
        name: request.name.clone(),
        network_volume_id: request.network_volume_id.clone(),
        ports: vec![PROVISIONER_PORT.to_owned()],
    })
}

fn endpoint_create_input(request: &CreateEndpointRequest) -> EndpointCreateInput {
    EndpointCreateInput {
        compute_type: "GPU",
        data_center_ids: vec![request.datacenter_id.clone()],
        gpu_type_ids: vec![request.gpu_id.clone()],
        name: request.name.clone(),
        network_volume_id: request.network_volume_id.clone(),
        template_id: request.template_id.clone(),
        workers_max: request.workers_max,
        workers_min: request.workers_min,
    }
}

fn template_create_input(request: &CreateTemplateRequest) -> TemplateCreateInput {
    TemplateCreateInput {
        image_name: request.image_ref.clone(),
        is_public: false,
        is_serverless: true,
        name: request.name.clone(),
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

#[derive(serde::Deserialize)]
struct CreatedResourceResponse {
    id: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkVolumeCreateInput {
    data_center_id: String,
    name: String,
    size: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PodCreateInput {
    compute_type: &'static str,
    data_center_ids: Vec<String>,
    env: HashMap<String, String>,
    image_name: String,
    name: String,
    network_volume_id: String,
    ports: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EndpointCreateInput {
    compute_type: &'static str,
    data_center_ids: Vec<String>,
    gpu_type_ids: Vec<String>,
    name: String,
    network_volume_id: String,
    template_id: String,
    workers_max: i64,
    workers_min: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateCreateInput {
    image_name: String,
    is_public: bool,
    is_serverless: bool,
    name: String,
}

#[cfg(test)]
mod tests {
    use crate::providers::runpod::{
        EndpointSummary, NetworkVolumeSummary, PodSummary, TemplateSummary,
    };

    #[test]
    fn runpod_list_payloads_deserialize_the_observation_fields() {
        let volumes: Vec<NetworkVolumeSummary> = serde_json::from_str(
            r#"[{"id":"volume-1","name":"volume","dataCenterId":"dc-1","size":19}]"#,
        )
        .unwrap();
        assert_eq!(volumes[0].id, "volume-1");
        assert_eq!(volumes[0].name, "volume");
        assert_eq!(volumes[0].data_center_id, "dc-1");
        assert_eq!(volumes[0].size, 19);

        let pods: Vec<PodSummary> = serde_json::from_str(
            r#"[{"id":"pod-1","name":"pod","networkVolume":{"id":"volume-1"}}]"#,
        )
        .unwrap();
        assert_eq!(pods[0].id, "pod-1");
        assert_eq!(pods[0].name, "pod");
        assert_eq!(pods[0].network_volume.as_ref().unwrap().id, "volume-1");

        let templates: Vec<TemplateSummary> = serde_json::from_str(
            r#"[{"id":"template-1","name":"template","isPublic":false,"isServerless":true}]"#,
        )
        .unwrap();
        assert_eq!(templates[0].id, "template-1");
        assert_eq!(templates[0].name, "template");
        assert!(!templates[0].is_public);
        assert!(templates[0].is_serverless);

        let endpoints: Vec<EndpointSummary> = serde_json::from_str(
            r#"[{"id":"endpoint-1","name":"endpoint","gpuTypeIds":["gpu-1"],"networkVolumeId":"volume-1","templateId":"template-1"}]"#,
        )
        .unwrap();
        assert_eq!(endpoints[0].id, "endpoint-1");
        assert_eq!(endpoints[0].name, "endpoint");
        assert_eq!(endpoints[0].gpu_type_ids, ["gpu-1"]);
        assert_eq!(endpoints[0].network_volume_id.as_deref(), Some("volume-1"));
        assert_eq!(endpoints[0].template_id.as_deref(), Some("template-1"));
    }
}
