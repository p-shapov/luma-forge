use crate::{
    domain::secrets::ApiKeyIdentity,
    provider::errors::ProviderApiError,
    secrets::{
        errors::{identity_response_invalid_message, SecretsStorageError},
        stores::ApiSecret,
        ApiKeyIdentityProvider,
    },
};

use super::client::{
    GraphQlError, GraphQlResponse, RunpodApiClient, RunpodApiKey, RunpodIdentityData,
};

#[derive(Clone)]
pub struct RunpodIdentityProvider<C = RunpodApiClient> {
    client: C,
}

#[async_trait::async_trait]
pub(super) trait RunpodIdentityClient: Clone + Send + Sync {
    async fn identity(&self, secret: &ApiSecret) -> Result<ApiKeyIdentity, SecretsStorageError>;
}

#[async_trait::async_trait]
impl RunpodIdentityClient for RunpodApiClient {
    async fn identity(&self, secret: &ApiSecret) -> Result<ApiKeyIdentity, SecretsStorageError> {
        self.get_identity(secret.expose_secret().to_string()).await
    }
}

impl RunpodIdentityProvider {
    pub fn new() -> Result<Self, SecretsStorageError> {
        Ok(Self {
            client: RunpodApiClient::new()?,
        })
    }
}

#[async_trait::async_trait]
impl<C> ApiKeyIdentityProvider for RunpodIdentityProvider<C>
where
    C: RunpodIdentityClient,
{
    async fn identity(&self, secret: &ApiSecret) -> Result<ApiKeyIdentity, SecretsStorageError> {
        self.client.identity(secret).await
    }
}

pub(super) fn map_graphql_response(
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
            ProviderApiError::Unauthorized,
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
        SecretsStorageError::IdentityRequestFailed(ProviderApiError::Unauthorized)
    } else {
        identity_response_invalid_message("API key is invalid")
    }
}

#[cfg(test)]
mod tests {
    use crate::{domain::secrets::ApiKeyIdentity, secrets::SecretsStorageError};

    use super::super::client::{
        GraphQlError, GraphQlResponse, RunpodApiKey, RunpodIdentity, RunpodIdentityData,
    };
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
                ProviderApiError::Unauthorized
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
                ProviderApiError::Unauthorized
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
}
