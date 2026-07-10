use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::infra::clients::{http, http::ResponseExt, NetworkError};

use super::HuggingFaceIdentity;

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

    pub async fn get_identity(
        &self,
        token: &SecretString,
    ) -> Result<HuggingFaceIdentity, NetworkError> {
        let response = self
            .http
            .get(WHOAMI_URL)
            .bearer_auth(token.expose_secret())
            .send()
            .await
            .into_json::<WhoamiResponse>()
            .await?;

        map_identity(response)
    }
}

#[derive(Deserialize)]
struct WhoamiResponse {
    name: String,
    email: Option<String>,
    auth: Option<WhoamiAuth>,
}

#[derive(Deserialize)]
struct WhoamiAuth {
    #[serde(rename = "accessToken")]
    access_token: Option<WhoamiAccessToken>,
}

#[derive(Deserialize)]
struct WhoamiAccessToken {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

fn map_identity(response: WhoamiResponse) -> Result<HuggingFaceIdentity, NetworkError> {
    let username = response.name.trim();
    if username.is_empty() {
        return Err(NetworkError::InvalidResponse);
    }

    Ok(HuggingFaceIdentity {
        username: username.to_owned(),
        email: normalized(response.email),
        token_display_name: response
            .auth
            .and_then(|auth| auth.access_token)
            .and_then(|token| normalized(token.display_name)),
    })
}

fn normalized(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
