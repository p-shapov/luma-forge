use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{domain::secrets::ApiKeyIdentity, shared::AppFuture};

use super::{errors::SecretsStorageError, identity::ApiKeyIdentityProvider, store::ApiSecret};

const RUNPOD_GRAPHQL_ENDPOINT: &str = "https://api.runpod.io/graphql";
const RUNPOD_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RUNPOD_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const RUNPOD_IDENTITY_QUERY: &str =
    "query LumaForgeRunpodIdentity { myself { email apiKeys { id isActive } } }";

pub struct RunpodIdentityProvider {
    http: reqwest::Client,
    endpoint: String,
}

impl RunpodIdentityProvider {
    pub fn try_new_default() -> Result<Self, SecretsStorageError> {
        let http = reqwest::Client::builder()
            .connect_timeout(RUNPOD_CONNECT_TIMEOUT)
            .timeout(RUNPOD_REQUEST_TIMEOUT)
            .build()
            .map_err(|_| SecretsStorageError::ProviderUnavailable)?;

        Ok(Self {
            http,
            endpoint: RUNPOD_GRAPHQL_ENDPOINT.to_string(),
        })
    }

    pub async fn fetch_identity(
        &self,
        secret: &ApiSecret,
    ) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(secret.expose_secret())
            .json(&GraphQlRequest {
                query: RUNPOD_IDENTITY_QUERY,
            })
            .send()
            .await
            .map_err(|_| SecretsStorageError::ProviderUnavailable)?;

        if let Some(error) = map_status_error(response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphQlResponse<RunpodIdentityData>>()
            .await
            .map_err(|_| SecretsStorageError::IdentityResponseInvalid)?;

        map_graphql_response(response)
    }
}

impl ApiKeyIdentityProvider for RunpodIdentityProvider {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
        Box::pin(async move { self.fetch_identity(secret).await })
    }
}

#[derive(Serialize)]
struct GraphQlRequest<'a> {
    query: &'a str,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    #[allow(dead_code)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct RunpodIdentityData {
    myself: Option<RunpodIdentity>,
}

#[derive(Debug, Deserialize)]
struct RunpodIdentity {
    email: String,
    #[serde(rename = "apiKeys")]
    api_keys: Vec<RunpodApiKey>,
}

#[derive(Debug, Deserialize)]
struct RunpodApiKey {
    id: Option<String>,
    #[serde(rename = "isActive")]
    is_active: bool,
}

fn map_status_error(status: StatusCode) -> Option<SecretsStorageError> {
    if status.is_success() {
        return None;
    }

    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Some(SecretsStorageError::Unauthorized),
        StatusCode::TOO_MANY_REQUESTS => Some(SecretsStorageError::RateLimited),
        _ => Some(SecretsStorageError::ProviderUnavailable),
    }
}

fn map_graphql_response(
    response: GraphQlResponse<RunpodIdentityData>,
) -> Result<ApiKeyIdentity, SecretsStorageError> {
    if !response.errors.is_empty() {
        return Err(SecretsStorageError::IdentityResponseInvalid);
    }

    let identity = response
        .data
        .and_then(|data| data.myself)
        .ok_or(SecretsStorageError::IdentityResponseInvalid)?;

    let email = identity.email.trim();
    if email.is_empty() {
        return Err(SecretsStorageError::IdentityResponseInvalid);
    }

    let active_key = identity
        .api_keys
        .iter()
        .find(|api_key| api_key.is_active)
        .ok_or(SecretsStorageError::IdentityResponseInvalid)?;

    Ok(ApiKeyIdentity {
        email: Some(email.to_string()),
        username: None,
        key_display_name: active_key
            .id
            .as_ref()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use crate::domain::secrets::ApiKeyIdentity;

    use super::*;

    #[test]
    fn maps_valid_identity_response() {
        let response = GraphQlResponse {
            data: Some(RunpodIdentityData {
                myself: Some(RunpodIdentity {
                    email: "user@example.com".to_string(),
                    api_keys: vec![
                        RunpodApiKey {
                            id: Some("inactive-key".to_string()),
                            is_active: false,
                        },
                        RunpodApiKey {
                            id: Some("active-key".to_string()),
                            is_active: true,
                        },
                    ],
                }),
            }),
            errors: Vec::new(),
        };

        assert_eq!(
            map_graphql_response(response),
            Ok(ApiKeyIdentity {
                email: Some("user@example.com".to_string()),
                username: None,
                key_display_name: Some("active-key".to_string()),
            })
        );
    }

    #[test]
    fn rejects_missing_active_key() {
        let response = GraphQlResponse {
            data: Some(RunpodIdentityData {
                myself: Some(RunpodIdentity {
                    email: "user@example.com".to_string(),
                    api_keys: vec![RunpodApiKey {
                        id: Some("inactive-key".to_string()),
                        is_active: false,
                    }],
                }),
            }),
            errors: Vec::new(),
        };

        assert_eq!(
            map_graphql_response(response),
            Err(SecretsStorageError::IdentityResponseInvalid)
        );
    }

    #[test]
    fn rejects_graphql_errors() {
        let response = GraphQlResponse::<RunpodIdentityData> {
            data: None,
            errors: vec![GraphQlError {
                message: "invalid token".to_string(),
            }],
        };

        assert_eq!(
            map_graphql_response(response),
            Err(SecretsStorageError::IdentityResponseInvalid)
        );
    }

    #[test]
    fn maps_unauthorized_status() {
        assert_eq!(
            map_status_error(StatusCode::UNAUTHORIZED),
            Some(SecretsStorageError::Unauthorized)
        );
        assert_eq!(
            map_status_error(StatusCode::FORBIDDEN),
            Some(SecretsStorageError::Unauthorized)
        );
    }
}
