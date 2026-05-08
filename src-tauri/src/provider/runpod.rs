use serde::{Deserialize, Serialize};

use crate::{
    domain::provider_setup::{ProviderApiKey, ProviderIdentity},
    provider_setup::ProviderSetupError,
};

const RUNPOD_GRAPHQL_ENDPOINT: &str = "https://api.runpod.io/graphql";
const IDENTITY_QUERY: &str = r#"
query LumaForgeProviderIdentity {
  myself {
    email
    apiKeys {
      id
      isActive
    }
  }
}
"#;

#[derive(Debug, Clone)]
pub struct RunPodClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for RunPodClient {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
            endpoint: RUNPOD_GRAPHQL_ENDPOINT.to_string(),
        }
    }
}

impl RunPodClient {
    pub async fn validate_identity(
        &self,
        api_key: &ProviderApiKey,
    ) -> Result<ProviderIdentity, ProviderSetupError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphQlRequest {
                query: IDENTITY_QUERY,
            })
            .send()
            .await
            .map_err(|_| ProviderSetupError::ProviderApiUnavailable)?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderSetupError::InvalidProviderApiKey);
        }
        if !status.is_success() {
            return Err(ProviderSetupError::ProviderApiUnavailable);
        }

        let payload = response
            .json::<GraphQlResponse<RunPodIdentityData>>()
            .await
            .map_err(|_| ProviderSetupError::ProviderIdentityUnavailable)?;

        identity_from_graphql_response(api_key, payload)
    }
}

#[derive(Debug, Serialize)]
struct GraphQlRequest<'a> {
    query: &'a str,
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct RunPodIdentityData {
    myself: Option<RunPodUser>,
}

#[derive(Debug, Deserialize)]
struct RunPodUser {
    email: Option<String>,
    #[serde(rename = "apiKeys")]
    api_keys: Option<Vec<RunPodApiKey>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct RunPodApiKey {
    id: Option<String>,
    #[serde(rename = "isActive")]
    is_active: Option<bool>,
}

fn identity_from_graphql_response(
    api_key: &ProviderApiKey,
    payload: GraphQlResponse<RunPodIdentityData>,
) -> Result<ProviderIdentity, ProviderSetupError> {
    if let Some(errors) = payload.errors.filter(|errors| !errors.is_empty()) {
        return Err(classify_graphql_errors(&errors));
    }

    let myself = payload
        .data
        .and_then(|data| data.myself)
        .ok_or(ProviderSetupError::ProviderIdentityUnavailable)?;
    let provider_user_email = myself
        .email
        .filter(|email| !email.is_empty())
        .ok_or(ProviderSetupError::ProviderIdentityUnavailable)?;
    let api_keys = myself
        .api_keys
        .ok_or(ProviderSetupError::ProviderIdentityUnavailable)?;
    let matched_api_key = match_api_key(api_key.expose_secret(), &api_keys)?;

    if matched_api_key.is_active != Some(true) {
        return Err(ProviderSetupError::InvalidProviderApiKey);
    }

    Ok(ProviderIdentity {
        provider_user_email,
        provider_api_key_fingerprint: matched_api_key
            .id
            .clone()
            .ok_or(ProviderSetupError::ProviderIdentityUnavailable)?,
    })
}

fn match_api_key<'a>(
    secret: &str,
    api_keys: &'a [RunPodApiKey],
) -> Result<&'a RunPodApiKey, ProviderSetupError> {
    let mut matches = api_keys
        .iter()
        .filter(|api_key| {
            api_key
                .id
                .as_ref()
                .is_some_and(|id| !id.is_empty() && secret.starts_with(id))
        })
        .take(2);

    let Some(first) = matches.next() else {
        return Err(ProviderSetupError::ProviderIdentityUnavailable);
    };
    if matches.next().is_some() {
        return Err(ProviderSetupError::ProviderIdentityUnavailable);
    }

    Ok(first)
}

fn classify_graphql_errors(errors: &[GraphQlError]) -> ProviderSetupError {
    if errors.iter().any(|error| {
        let message = error.message.to_ascii_lowercase();
        message.contains("unauthorized")
            || message.contains("forbidden")
            || message.contains("unauthenticated")
            || message.contains("authentication")
            || message.contains("api key")
    }) {
        ProviderSetupError::InvalidProviderApiKey
    } else {
        ProviderSetupError::ProviderApiUnavailable
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse_identity(
        secret: &str,
        value: serde_json::Value,
    ) -> Result<ProviderIdentity, ProviderSetupError> {
        let response: GraphQlResponse<RunPodIdentityData> =
            serde_json::from_value(value).expect("response should parse");
        identity_from_graphql_response(&ProviderApiKey::new(secret.to_string()).unwrap(), response)
    }

    #[test]
    fn parses_identity_with_single_active_prefix_match() {
        let identity = parse_identity(
            "rp_123_secret",
            json!({
                "data": {
                    "myself": {
                        "email": "user@example.com",
                        "apiKeys": [
                            { "id": "rp_123", "isActive": true },
                            { "id": "rp_999", "isActive": true }
                        ]
                    }
                }
            }),
        )
        .expect("identity should parse");

        assert_eq!(identity.provider_user_email, "user@example.com");
        assert_eq!(identity.provider_api_key_fingerprint, "rp_123");
    }

    #[test]
    fn rejects_inactive_matched_key() {
        let error = parse_identity(
            "rp_123_secret",
            json!({
                "data": {
                    "myself": {
                        "email": "user@example.com",
                        "apiKeys": [
                            { "id": "rp_123", "isActive": false }
                        ]
                    }
                }
            }),
        )
        .expect_err("inactive key should fail");

        assert_eq!(error, ProviderSetupError::InvalidProviderApiKey);
    }

    #[test]
    fn rejects_missing_prefix_match() {
        let error = parse_identity(
            "rp_123_secret",
            json!({
                "data": {
                    "myself": {
                        "email": "user@example.com",
                        "apiKeys": [
                            { "id": "rp_999", "isActive": true }
                        ]
                    }
                }
            }),
        )
        .expect_err("missing match should fail");

        assert_eq!(error, ProviderSetupError::ProviderIdentityUnavailable);
    }

    #[test]
    fn rejects_ambiguous_prefix_match() {
        let error = parse_identity(
            "rp_123_secret",
            json!({
                "data": {
                    "myself": {
                        "email": "user@example.com",
                        "apiKeys": [
                            { "id": "rp_", "isActive": true },
                            { "id": "rp_123", "isActive": true }
                        ]
                    }
                }
            }),
        )
        .expect_err("ambiguous match should fail");

        assert_eq!(error, ProviderSetupError::ProviderIdentityUnavailable);
    }

    #[test]
    fn maps_auth_graphql_errors_to_invalid_key() {
        let error = parse_identity(
            "rp_123_secret",
            json!({
                "errors": [
                    { "message": "Unauthorized" }
                ]
            }),
        )
        .expect_err("auth errors should fail");

        assert_eq!(error, ProviderSetupError::InvalidProviderApiKey);
    }
}
