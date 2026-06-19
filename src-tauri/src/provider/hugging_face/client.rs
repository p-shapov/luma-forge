use std::time::Duration;

use crate::{
    domain::secrets::ApiKeyIdentity,
    secrets::errors::{
        identity_request_error, identity_response_invalid_error, identity_status_error,
        SecretsStorageError,
    },
};

use super::identity::map_whoami_response;

const HUGGING_FACE_WHOAMI_ENDPOINT: &str = "https://huggingface.co/api/whoami-v2";
pub(super) const HUGGING_FACE_PROVIDER_NAME: &str = "Hugging Face";
const HUGGING_FACE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HUGGING_FACE_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct HuggingFaceApiClient {
    http: reqwest::Client,
    whoami_endpoint: String,
}

impl HuggingFaceApiClient {
    pub(super) fn new() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(HUGGING_FACE_CONNECT_TIMEOUT)
                .timeout(HUGGING_FACE_REQUEST_TIMEOUT)
                .build()
                .map_err(identity_request_error)?,
            whoami_endpoint: HUGGING_FACE_WHOAMI_ENDPOINT.to_string(),
        })
    }

    pub(super) async fn get_identity(
        &self,
        api_key: String,
    ) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let response = self
            .http
            .get(&self.whoami_endpoint)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(identity_request_error)?;

        if let Some(error) = identity_status_error(HUGGING_FACE_PROVIDER_NAME, response.status()) {
            return Err(error);
        }

        let response = response
            .json::<serde_json::Value>()
            .await
            .map_err(identity_response_invalid_error)?;

        map_whoami_response(response)
    }
}
