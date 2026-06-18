use std::time::Duration;

use serde::Deserialize;

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

const HUGGING_FACE_WHOAMI_ENDPOINT: &str = "https://huggingface.co/api/whoami-v2";
const HUGGING_FACE_PROVIDER_NAME: &str = "Hugging Face";
const HUGGING_FACE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HUGGING_FACE_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct HuggingFaceIdentityProvider {
    http: reqwest::Client,
    whoami_endpoint: String,
}

impl HuggingFaceIdentityProvider {
    pub fn try_new_default() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            http: identity_http_client(HUGGING_FACE_CONNECT_TIMEOUT, HUGGING_FACE_REQUEST_TIMEOUT)?,
            whoami_endpoint: HUGGING_FACE_WHOAMI_ENDPOINT.to_string(),
        })
    }

    pub async fn fetch_identity(
        &self,
        secret: &ApiSecret,
    ) -> Result<ApiKeyIdentity, SecretsStorageError> {
        let response = self
            .http
            .get(&self.whoami_endpoint)
            .bearer_auth(secret.expose_secret())
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

impl ApiKeyIdentityProvider for HuggingFaceIdentityProvider {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
        Box::pin(async move { self.fetch_identity(secret).await })
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
    fine_grained: Option<WhoamiFineGrainedPermissions>,
}

#[derive(Debug, Deserialize)]
struct WhoamiFineGrainedPermissions {
    #[serde(rename = "canReadGatedRepos")]
    can_read_gated_repos: Option<bool>,
    #[serde(default)]
    global: Vec<String>,
    #[serde(default)]
    scoped: Vec<WhoamiFineGrainedRepoPermissions>,
}

#[derive(Debug, Deserialize)]
struct WhoamiFineGrainedRepoPermissions {
    #[serde(default)]
    permissions: Vec<String>,
}

fn map_whoami_response(response: serde_json::Value) -> Result<ApiKeyIdentity, SecretsStorageError> {
    let response = serde_json::from_value::<WhoamiResponse>(response)
        .map_err(identity_response_invalid_error)?;

    let name = response.name.trim();
    if name.is_empty() {
        return Err(identity_response_invalid_message("identity name is empty"));
    }

    let display_name = response.auth.access_token.display_name.trim();
    if display_name.is_empty() {
        return Err(identity_response_invalid_message(
            "identity display name is empty",
        ));
    }

    match response.auth.access_token.role.as_str() {
        "read" | "write" => {}
        "fineGrained" => {
            let fine_grained = response.auth.access_token.fine_grained.ok_or(
                SecretsStorageError::IdentityRequestFailed(ApiError::InsufficientPermissions),
            )?;

            if !fine_grained.can_read_gated_repos.unwrap_or(false)
                || !fine_grained.has_repo_content_read_permission()
            {
                return Err(SecretsStorageError::IdentityRequestFailed(
                    ApiError::InsufficientPermissions,
                ));
            }
        }
        _ => {
            return Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions,
            ));
        }
    }

    Ok(ApiKeyIdentity {
        email: response
            .email
            .as_ref()
            .map(|email| email.trim())
            .filter(|email| !email.is_empty())
            .map(ToOwned::to_owned),
        username: Some(name.to_string()),
        key_display_name: Some(display_name.to_string()),
    })
}

impl WhoamiFineGrainedPermissions {
    fn has_repo_content_read_permission(&self) -> bool {
        self.global
            .iter()
            .any(|permission| permission == "repo.content.read")
            || self.scoped.iter().any(|repo_permission| {
                repo_permission
                    .permissions
                    .iter()
                    .any(|permission| permission == "repo.content.read")
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::domain::secrets::ApiKeyIdentity;

    use super::*;

    #[test]
    fn maps_valid_read_token_with_missing_email() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge read token",
                    "role": "read"
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Ok(ApiKeyIdentity {
                email: None,
                username: Some("hf-user".to_string()),
                key_display_name: Some("LumaForge read token".to_string()),
            })
        );
    }

    #[test]
    fn maps_valid_write_token() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge write token",
                    "role": "write"
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Ok(ApiKeyIdentity {
                email: None,
                username: Some("hf-user".to_string()),
                key_display_name: Some("LumaForge write token".to_string()),
            })
        );
    }

    #[test]
    fn accepts_fine_grained_token_with_required_permissions() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge fine token",
                    "role": "fineGrained",
                    "fineGrained": {
                        "canReadGatedRepos": true,
                        "global": ["repo.content.read"]
                    }
                }
            },
            "name": "hf-user",
            "email": " user@example.com "
        });

        assert_eq!(
            map_whoami_response(response),
            Ok(ApiKeyIdentity {
                email: Some("user@example.com".to_string()),
                username: Some("hf-user".to_string()),
                key_display_name: Some("LumaForge fine token".to_string()),
            })
        );
    }

    #[test]
    fn accepts_fine_grained_token_with_scoped_permissions() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge fine token",
                    "role": "fineGrained",
                    "fineGrained": {
                        "canReadGatedRepos": true,
                        "scoped": [
                            {
                                "permissions": ["repo.content.read"]
                            }
                        ]
                    }
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Ok(ApiKeyIdentity {
                email: None,
                username: Some("hf-user".to_string()),
                key_display_name: Some("LumaForge fine token".to_string()),
            })
        );
    }

    #[test]
    fn rejects_fine_grained_token_missing_fine_grained_permissions() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge fine token",
                    "role": "fineGrained"
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions
            ))
        );
    }

    #[test]
    fn rejects_fine_grained_token_without_gated_repo_read() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge fine token",
                    "role": "fineGrained",
                    "fineGrained": {
                        "canReadGatedRepos": false,
                        "global": ["repo.content.read"]
                    }
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions
            ))
        );
    }

    #[test]
    fn rejects_fine_grained_token_without_required_permissions() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge fine token",
                    "role": "fineGrained",
                    "fineGrained": {
                        "canReadGatedRepos": true,
                        "global": ["repo.write"]
                    }
                }
            },
            "name": "hf-user"
        });

        assert_eq!(
            map_whoami_response(response),
            Err(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions
            ))
        );
    }

    #[test]
    fn rejects_blank_name() {
        let response = json!({
            "auth": {
                "accessToken": {
                    "displayName": "LumaForge read token",
                    "role": "read"
                }
            },
            "name": " \n\t "
        });

        assert_eq!(
            map_whoami_response(response),
            Err(SecretsStorageError::IdentityResponseInvalid {
                message: "identity name is empty".to_string()
            })
        );
    }

    #[test]
    fn maps_unauthorized_status() {
        assert_eq!(
            identity_status_error(
                HUGGING_FACE_PROVIDER_NAME,
                reqwest::StatusCode::UNAUTHORIZED
            ),
            Some(SecretsStorageError::IdentityRequestFailed(
                ApiError::Unauthorized
            ))
        );
        assert_eq!(
            identity_status_error(HUGGING_FACE_PROVIDER_NAME, reqwest::StatusCode::FORBIDDEN),
            Some(SecretsStorageError::IdentityRequestFailed(
                ApiError::InsufficientPermissions
            ))
        );
    }
}
