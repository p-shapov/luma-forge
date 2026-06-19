use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    domain::secrets::ApiKeyIdentity,
    shared::{ApiError, AppFuture},
};

use crate::secrets::{
    errors::{
        identity_request_error, identity_response_invalid_error, identity_response_invalid_message,
        identity_status_error, SecretsStorageError,
    },
    identities::identity_http_client,
    stores::ApiSecret,
    ApiKeyIdentityProvider,
};

const RUNPOD_GRAPHQL_ENDPOINT: &str = "https://api.runpod.io/graphql";
const RUNPOD_PROVIDER_NAME: &str = "RunPod";
const RUNPOD_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RUNPOD_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const RUNPOD_IDENTITY_QUERY: &str =
    "query LumaForgeRunpodIdentity { myself { email apiKeys { id isActive } } }";

#[derive(Clone)]
pub struct RunpodIdentityProvider {
    http: reqwest::Client,
    endpoint: String,
}

impl RunpodIdentityProvider {
    pub fn try_new_default() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            http: identity_http_client(RUNPOD_CONNECT_TIMEOUT, RUNPOD_REQUEST_TIMEOUT)?,
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
            .map_err(identity_request_error)?;

        if let Some(error) = identity_status_error(RUNPOD_PROVIDER_NAME, response.status()) {
            return Err(error);
        }

        let response = response
            .json::<GraphQlResponse<RunpodIdentityData>>()
            .await
            .map_err(identity_response_invalid_error)?;

        map_graphql_response(secret.expose_secret(), response)
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

fn map_graphql_response(
    submitted_secret: &str,
    response: GraphQlResponse<RunpodIdentityData>,
) -> Result<ApiKeyIdentity, SecretsStorageError> {
    if !response.errors.is_empty() {
        return Err(classify_graphql_errors(&response.errors));
    }

    let identity = response
        .data
        .and_then(|data| data.myself)
        .ok_or_else(|| identity_response_invalid_message("identity is missing"))?;

    let email = identity.email.trim();
    if email.is_empty() {
        return Err(identity_response_invalid_message("email is empty"));
    }

    let matched_key = match_api_key(submitted_secret, &identity.api_keys)?;
    if !matched_key.is_active {
        return Err(SecretsStorageError::IdentityRequestFailed(
            ApiError::Unauthorized,
        ));
    }

    Ok(ApiKeyIdentity {
        email: Some(email.to_string()),
        username: None,
        key_display_name: matched_key
            .id
            .as_ref()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned),
    })
}

fn match_api_key<'a>(
    submitted_secret: &str,
    api_keys: &'a [RunpodApiKey],
) -> Result<&'a RunpodApiKey, SecretsStorageError> {
    let mut matches = api_keys
        .iter()
        .filter(|api_key| {
            api_key
                .id
                .as_ref()
                .is_some_and(|id| !id.trim().is_empty() && submitted_secret.starts_with(id.trim()))
        })
        .take(2);

    let Some(first) = matches.next() else {
        return Err(identity_response_invalid_message(
            "no matching API key found",
        ));
    };

    if matches.next().is_some() {
        return Err(identity_response_invalid_message(
            "multiple matching API keys found",
        ));
    }

    Ok(first)
}

fn classify_graphql_errors(errors: &[GraphQlError]) -> SecretsStorageError {
    if errors.iter().any(|error| {
        let message = error.message.to_ascii_lowercase();
        message.contains("unauthorized")
            || message.contains("forbidden")
            || message.contains("unauthenticated")
            || message.contains("authentication")
            || message.contains("api key")
    }) {
        SecretsStorageError::IdentityRequestFailed(ApiError::Unauthorized)
    } else {
        identity_response_invalid_message("API key is invalid")
    }
}

#[cfg(test)]
mod tests {
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
            map_graphql_response("active-key-secret-value", response),
            Ok(ApiKeyIdentity {
                email: Some("user@example.com".to_string()),
                username: None,
                key_display_name: Some("active-key".to_string()),
            })
        );
    }

    #[test]
    fn rejects_when_no_key_matches_submitted_secret() {
        let response = GraphQlResponse {
            data: Some(RunpodIdentityData {
                myself: Some(RunpodIdentity {
                    email: "user@example.com".to_string(),
                    api_keys: vec![RunpodApiKey {
                        id: Some("other-inactive-key".to_string()),
                        is_active: false,
                    }],
                }),
            }),
            errors: Vec::new(),
        };

        assert_eq!(
            map_graphql_response("submitted-key-secret-value", response),
            Err(SecretsStorageError::IdentityResponseInvalid {
                message: "no matching API key found".to_string()
            })
        );
    }

    #[test]
    fn rejects_missing_matching_key() {
        let response = GraphQlResponse {
            data: Some(RunpodIdentityData {
                myself: Some(RunpodIdentity {
                    email: "user@example.com".to_string(),
                    api_keys: vec![RunpodApiKey {
                        id: Some("different-key".to_string()),
                        is_active: true,
                    }],
                }),
            }),
            errors: Vec::new(),
        };

        assert_eq!(
            map_graphql_response("submitted-key-secret-value", response),
            Err(SecretsStorageError::IdentityResponseInvalid {
                message: "no matching API key found".to_string()
            })
        );
    }

    #[test]
    fn rejects_inactive_matching_key_as_unauthorized() {
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
            map_graphql_response("inactive-key-secret-value", response),
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::Unauthorized
            ))
        );
    }

    #[test]
    fn rejects_ambiguous_matching_keys() {
        let response = GraphQlResponse {
            data: Some(RunpodIdentityData {
                myself: Some(RunpodIdentity {
                    email: "user@example.com".to_string(),
                    api_keys: vec![
                        RunpodApiKey {
                            id: Some("rp".to_string()),
                            is_active: true,
                        },
                        RunpodApiKey {
                            id: Some("rp_secret".to_string()),
                            is_active: true,
                        },
                    ],
                }),
            }),
            errors: Vec::new(),
        };

        assert_eq!(
            map_graphql_response("rp_secret_value", response),
            Err(SecretsStorageError::IdentityResponseInvalid {
                message: "multiple matching API keys found".to_string()
            })
        );
    }

    #[test]
    fn classifies_auth_graphql_errors_as_unauthorized() {
        let response = GraphQlResponse::<RunpodIdentityData> {
            data: None,
            errors: vec![GraphQlError {
                message: "API key is invalid".to_string(),
            }],
        };

        assert_eq!(
            map_graphql_response("submitted-key-secret-value", response),
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::Unauthorized
            ))
        );
    }

    #[test]
    fn rejects_non_auth_graphql_errors_as_invalid_response() {
        let response = GraphQlResponse::<RunpodIdentityData> {
            data: None,
            errors: vec![GraphQlError {
                message: "Cannot query field unknown".to_string(),
            }],
        };

        assert_eq!(
            map_graphql_response("submitted-key-secret-value", response),
            Err(SecretsStorageError::IdentityResponseInvalid {
                message: "API key is invalid".to_string()
            })
        );
    }

    #[test]
    fn maps_unauthorized_status() {
        assert_eq!(
            identity_status_error(RUNPOD_PROVIDER_NAME, reqwest::StatusCode::UNAUTHORIZED),
            Some(SecretsStorageError::IdentityRequestFailed(
                ApiError::Unauthorized
            ))
        );
        assert_eq!(
            identity_status_error(RUNPOD_PROVIDER_NAME, reqwest::StatusCode::FORBIDDEN),
            Some(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions
            ))
        );
    }
}
