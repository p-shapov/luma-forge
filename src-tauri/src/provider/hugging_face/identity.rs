use serde::Deserialize;

use crate::{
    domain::secrets::ApiKeyIdentity,
    secrets::{
        errors::{
            identity_response_invalid_error, identity_response_invalid_message, SecretsStorageError,
        },
        stores::ApiSecret,
        ApiKeyIdentityProvider,
    },
    shared::{ApiError, AppFuture},
};

use super::client::HuggingFaceApiClient;

#[derive(Clone)]
pub struct HuggingFaceIdentityProvider<C = HuggingFaceApiClient> {
    client: C,
}

pub(super) trait HuggingFaceIdentityClient: Clone + Send + Sync {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>>;
}

impl HuggingFaceIdentityClient for HuggingFaceApiClient {
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
        Box::pin(async move { self.get_identity(secret.expose_secret().to_string()).await })
    }
}

impl HuggingFaceIdentityProvider {
    pub fn new() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            client: HuggingFaceApiClient::new()?,
        })
    }
}

impl<C> ApiKeyIdentityProvider for HuggingFaceIdentityProvider<C>
where
    C: HuggingFaceIdentityClient,
{
    fn identity<'a>(
        &'a self,
        secret: &'a ApiSecret,
    ) -> AppFuture<'a, Result<ApiKeyIdentity, SecretsStorageError>> {
        self.client.identity(secret)
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

pub(super) fn map_whoami_response(
    response: serde_json::Value,
) -> Result<ApiKeyIdentity, SecretsStorageError> {
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

    use crate::{domain::secrets::ApiKeyIdentity, secrets::SecretsStorageError};

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
}
