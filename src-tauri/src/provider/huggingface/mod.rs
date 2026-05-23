use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    domain::hugging_face_setup::{HuggingFaceApiKey, HuggingFaceApiKeySetup},
    provider::ProviderClientError,
};

const HUGGING_FACE_WHOAMI_ENDPOINT: &str = "https://huggingface.co/api/whoami-v2";
const HUGGING_FACE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HUGGING_FACE_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct HuggingFaceClient {
    http: reqwest::Client,
    whoami_endpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("Hugging Face HTTP client initialization failed")]
pub struct HuggingFaceHttpClientInitError;

impl HuggingFaceClient {
    pub fn try_new_default() -> Result<Self, HuggingFaceHttpClientInitError> {
        Self::try_new(
            HUGGING_FACE_WHOAMI_ENDPOINT.to_string(),
            HUGGING_FACE_CONNECT_TIMEOUT,
            HUGGING_FACE_REQUEST_TIMEOUT,
        )
    }

    fn try_new(
        whoami_endpoint: String,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, HuggingFaceHttpClientInitError> {
        Self::with_http_client(
            reqwest::Client::builder()
                .connect_timeout(connect_timeout)
                .timeout(request_timeout)
                .build()
                .map_err(|_| HuggingFaceHttpClientInitError)?,
            whoami_endpoint,
        )
    }

    fn with_http_client(
        http: reqwest::Client,
        whoami_endpoint: String,
    ) -> Result<Self, HuggingFaceHttpClientInitError> {
        Ok(Self {
            http,
            whoami_endpoint,
        })
    }

    pub async fn validate_identity(
        &self,
        api_key: &HuggingFaceApiKey,
    ) -> Result<HuggingFaceApiKeySetup, ProviderClientError> {
        let response = self
            .http
            .get(&self.whoami_endpoint)
            .bearer_auth(api_key.expose_secret())
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        if let Some(error) = provider_error_from_status(response.status()) {
            return Err(error);
        }

        let payload = response
            .json::<serde_json::Value>()
            .await
            .map_err(|_| ProviderClientError::ResponseInvalid)?;

        identity_from_whoami_response(payload)
    }
}

#[derive(Debug, Deserialize)]
struct WhoamiResponse {
    auth: WhoamiAuth,
    name: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WhoamiAuth {
    #[serde(rename = "accessToken")]
    access_token: WhoamiAccessToken,
}

#[derive(Debug, Deserialize)]
struct WhoamiAccessToken {
    #[serde(rename = "displayName")]
    display_name: String,
    role: String,
    #[serde(rename = "fineGrained")]
    fine_grained: Option<WhoamiFineGrained>,
}

#[derive(Debug, Deserialize)]
struct WhoamiFineGrained {
    #[serde(rename = "canReadGatedRepos")]
    can_read_gated_repos: Option<bool>,
    #[serde(default)]
    global: Vec<String>,
    #[serde(default)]
    scoped: Vec<WhoamiScopedPermissions>,
}

#[derive(Debug, Deserialize)]
struct WhoamiScopedPermissions {
    #[serde(default)]
    permissions: Vec<String>,
}

fn identity_from_whoami_response(
    payload: serde_json::Value,
) -> Result<HuggingFaceApiKeySetup, ProviderClientError> {
    let response: WhoamiResponse =
        serde_json::from_value(payload).map_err(|_| ProviderClientError::ResponseInvalid)?;
    if is_blank(&response.auth.access_token.display_name) || is_blank(&response.name) {
        return Err(ProviderClientError::ResponseInvalid);
    }
    if !token_can_download_models(&response.auth.access_token) {
        return Err(ProviderClientError::InsufficientPermissions);
    }

    Ok(HuggingFaceApiKeySetup {
        token_name: response.auth.access_token.display_name,
        user_name: response.name,
        user_email: response.email,
    })
}

fn token_can_download_models(access_token: &WhoamiAccessToken) -> bool {
    match access_token.role.as_str() {
        "read" | "write" => true,
        "fineGrained" => {
            let Some(fine_grained) = &access_token.fine_grained else {
                return false;
            };
            fine_grained.can_read_gated_repos == Some(true)
                && (fine_grained
                    .global
                    .iter()
                    .any(|permission| permission == "repo.content.read")
                    || fine_grained.scoped.iter().any(|scope| {
                        scope
                            .permissions
                            .iter()
                            .any(|permission| permission == "repo.content.read")
                    }))
        }
        _ => false,
    }
}

fn provider_error_from_status(status: StatusCode) -> Option<ProviderClientError> {
    if status.is_success() {
        return None;
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Some(ProviderClientError::Unauthorized);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Some(ProviderClientError::RateLimited);
    }
    if status.is_client_error() {
        return Some(ProviderClientError::RequestRejected);
    }
    Some(ProviderClientError::ApiUnavailable)
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::hugging_face_setup::HuggingFaceApiKeySetup;

    #[test]
    fn identity_from_whoami_response_maps_safe_setup_fields() {
        let payload = serde_json::json!({
            "auth": {
                "type": "access_token",
                "accessToken": {
                    "displayName": "RUNPOD_READ",
                    "role": "read",
                    "createdAt": "2026-05-23T00:00:00Z"
                }
            },
            "type": "user",
            "id": "user-id",
            "name": "pavel",
            "fullname": "Pavel",
            "email": "pavel@example.com",
            "avatarUrl": "https://example.com/avatar.png",
            "isPro": false,
            "orgs": []
        });

        assert_eq!(
            identity_from_whoami_response(payload),
            Ok(HuggingFaceApiKeySetup {
                token_name: "RUNPOD_READ".to_string(),
                user_name: "pavel".to_string(),
                user_email: Some("pavel@example.com".to_string()),
            })
        );
    }

    #[test]
    fn identity_from_whoami_response_accepts_missing_email() {
        let payload = serde_json::json!({
            "auth": {
                "type": "access_token",
                "accessToken": {
                    "displayName": "RUNPOD_READ",
                    "role": "read",
                    "createdAt": "2026-05-23T00:00:00Z"
                }
            },
            "type": "user",
            "id": "user-id",
            "name": "pavel",
            "fullname": "Pavel",
            "email": null,
            "avatarUrl": "https://example.com/avatar.png",
            "isPro": false,
            "orgs": []
        });

        assert_eq!(
            identity_from_whoami_response(payload)
                .expect("identity should parse")
                .user_email,
            None
        );
    }

    #[test]
    fn identity_from_whoami_response_accepts_fine_grained_download_flags() {
        let payload = serde_json::json!({
            "auth": {
                "type": "access_token",
                "accessToken": {
                    "displayName": "1",
                    "role": "fineGrained",
                    "createdAt": "2026-05-23T11:23:45.759Z",
                    "fineGrained": {
                        "canReadGatedRepos": true,
                        "global": [],
                        "scoped": [
                            {
                                "entity": {
                                    "_id": "6999f419d475051977e04a24",
                                    "type": "user",
                                    "name": "p-shapov"
                                },
                                "permissions": [
                                    "repo.content.read",
                                    "repo.access.read"
                                ]
                            }
                        ]
                    }
                }
            },
            "type": "user",
            "id": "user-id",
            "name": "p-shapov",
            "fullname": "Pavel Shapov",
            "avatarUrl": "https://example.com/avatar.png",
            "isPro": false,
            "orgs": []
        });

        assert_eq!(
            identity_from_whoami_response(payload),
            Ok(HuggingFaceApiKeySetup {
                token_name: "1".to_string(),
                user_name: "p-shapov".to_string(),
                user_email: None,
            })
        );
    }

    #[test]
    fn identity_from_whoami_response_rejects_fine_grained_token_without_download_flags() {
        for mutate in [
            |payload: &mut serde_json::Value| {
                payload["auth"]["accessToken"]["fineGrained"]["canReadGatedRepos"] =
                    serde_json::json!(false);
            },
            |payload: &mut serde_json::Value| {
                payload["auth"]["accessToken"]["fineGrained"]["scoped"][0]["permissions"] =
                    serde_json::json!(["repo.access.read"]);
            },
        ] {
            let mut payload = serde_json::json!({
                "auth": {
                    "type": "access_token",
                    "accessToken": {
                        "displayName": "1",
                        "role": "fineGrained",
                        "createdAt": "2026-05-23T11:23:45.759Z",
                        "fineGrained": {
                            "canReadGatedRepos": true,
                            "global": [],
                            "scoped": [
                                {
                                    "entity": {
                                        "_id": "6999f419d475051977e04a24",
                                        "type": "user",
                                        "name": "p-shapov"
                                    },
                                    "permissions": [
                                        "repo.content.read"
                                    ]
                                }
                            ]
                        }
                    }
                },
                "type": "user",
                "id": "user-id",
                "name": "p-shapov",
                "fullname": "Pavel Shapov",
                "avatarUrl": "https://example.com/avatar.png",
                "isPro": false,
                "orgs": []
            });
            mutate(&mut payload);

            assert_eq!(
                identity_from_whoami_response(payload),
                Err(ProviderClientError::InsufficientPermissions)
            );
        }
    }

    #[test]
    fn identity_from_whoami_response_rejects_missing_display_name() {
        let payload = serde_json::json!({
            "auth": {
                "type": "access_token",
                "accessToken": {
                    "displayName": " ",
                    "role": "read",
                    "createdAt": "2026-05-23T00:00:00Z"
                }
            },
            "type": "user",
            "id": "user-id",
            "name": "pavel",
            "fullname": "Pavel",
            "email": "pavel@example.com",
            "avatarUrl": "https://example.com/avatar.png",
            "isPro": false,
            "orgs": []
        });

        assert_eq!(
            identity_from_whoami_response(payload),
            Err(ProviderClientError::ResponseInvalid)
        );
    }

    #[test]
    fn hugging_face_statuses_map_to_provider_errors() {
        assert_eq!(
            provider_error_from_status(reqwest::StatusCode::UNAUTHORIZED),
            Some(ProviderClientError::Unauthorized)
        );
        assert_eq!(
            provider_error_from_status(reqwest::StatusCode::FORBIDDEN),
            Some(ProviderClientError::Unauthorized)
        );
        assert_eq!(
            provider_error_from_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            Some(ProviderClientError::RateLimited)
        );
        assert_eq!(
            provider_error_from_status(reqwest::StatusCode::BAD_REQUEST),
            Some(ProviderClientError::RequestRejected)
        );
        assert_eq!(
            provider_error_from_status(reqwest::StatusCode::SERVICE_UNAVAILABLE),
            Some(ProviderClientError::ApiUnavailable)
        );
        assert_eq!(provider_error_from_status(reqwest::StatusCode::OK), None);
    }
}
