use secrecy::{ExposeSecret, SecretString};

use crate::infra::clients::{http, http::ResponseExt, NetworkError};

use super::WhoamiResponse;

const WHOAMI_URL: &str = "https://huggingface.co/api/whoami-v2";

#[derive(Clone)]
pub struct HuggingFaceClient {
    http: reqwest::Client,
}

impl HuggingFaceClient {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            http: http::client()?,
        })
    }

    pub async fn whoami(&self, token: &SecretString) -> Result<WhoamiResponse, NetworkError> {
        self.http
            .get(WHOAMI_URL)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .into_json::<WhoamiResponse>()
            .await
    }
}
