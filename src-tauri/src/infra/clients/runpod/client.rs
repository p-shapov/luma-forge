use graphql_client::GraphQLQuery;
use reqwest::header::CONTENT_TYPE;
use secrecy::{ExposeSecret, SecretString};

use crate::infra::clients::{graphql::GraphqlResponseExt, http, http::ResponseExt, NetworkError};

use super::{
    queries::{myself, placement, Myself, Placement},
    Endpoint, EndpointCreateInput, MyselfResponse, NetworkVolume, NetworkVolumeCreateInput,
    PlacementResponse, Pod, PodCreateInput, Template, TemplateCreateInput,
};

const GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const REST_BASE_URL: &str = "https://rest.runpod.io/v1";

#[derive(serde::Deserialize)]
pub struct ProvisionerStatusResponse {
    pub status: String,
    pub error: Option<ProvisionerFailure>,
}

#[derive(serde::Deserialize)]
pub struct ProvisionerFailure {
    pub code: String,
    pub message: String,
}

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

    pub async fn myself(&self, api_key: &SecretString) -> Result<MyselfResponse, NetworkError> {
        self.http
            .post(GRAPHQL_URL)
            .bearer_auth(api_key.expose_secret())
            .header(CONTENT_TYPE, "application/json")
            .json(&Myself::build_query(myself::Variables {}))
            .send()
            .await
            .into_graphql_data()
            .await
    }

    pub async fn placement(
        &self,
        api_key: &SecretString,
    ) -> Result<PlacementResponse, NetworkError> {
        self.http
            .post(GRAPHQL_URL)
            .bearer_auth(api_key.expose_secret())
            .header(CONTENT_TYPE, "application/json")
            .json(&Placement::build_query(placement::Variables {}))
            .send()
            .await
            .into_graphql_data()
            .await
    }

    pub async fn create_network_volume(
        &self,
        api_key: &SecretString,
        request: NetworkVolumeCreateInput,
    ) -> Result<NetworkVolume, NetworkError> {
        self.http
            .post(format!("{REST_BASE_URL}/networkvolumes"))
            .bearer_auth(api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .into_json()
            .await
    }

    pub async fn delete_network_volume(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), NetworkError> {
        self.http
            .delete(format!("{REST_BASE_URL}/networkvolumes/{id}"))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .into_response()?;
        Ok(())
    }

    pub async fn create_pod(
        &self,
        api_key: &SecretString,
        request: PodCreateInput,
    ) -> Result<Pod, NetworkError> {
        self.http
            .post(format!("{REST_BASE_URL}/pods"))
            .bearer_auth(api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .into_json()
            .await
    }

    pub async fn provisioner_status(
        &self,
        bearer_token: &SecretString,
        pod_id: &str,
    ) -> Result<ProvisionerStatusResponse, NetworkError> {
        self.http
            .get(format!("https://{pod_id}-8000.proxy.runpod.net/status"))
            .bearer_auth(bearer_token.expose_secret())
            .send()
            .await
            .into_json()
            .await
    }

    pub async fn create_endpoint(
        &self,
        api_key: &SecretString,
        request: EndpointCreateInput,
    ) -> Result<Endpoint, NetworkError> {
        self.http
            .post(format!("{REST_BASE_URL}/endpoints"))
            .bearer_auth(api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .into_json()
            .await
    }

    pub async fn create_template(
        &self,
        api_key: &SecretString,
        request: TemplateCreateInput,
    ) -> Result<Template, NetworkError> {
        self.http
            .post(format!("{REST_BASE_URL}/templates"))
            .bearer_auth(api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .into_json()
            .await
    }

    pub async fn delete_pod(&self, api_key: &SecretString, id: &str) -> Result<(), NetworkError> {
        self.http
            .delete(format!("{REST_BASE_URL}/pods/{id}"))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .into_response()?;
        Ok(())
    }

    pub async fn delete_template(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), NetworkError> {
        self.http
            .delete(format!("{REST_BASE_URL}/templates/{id}"))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .into_response()?;
        Ok(())
    }

    pub async fn delete_endpoint(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), NetworkError> {
        self.http
            .delete(format!("{REST_BASE_URL}/endpoints/{id}"))
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .into_response()?;
        Ok(())
    }
}
