use std::time::Duration;

#[cfg(test)]
use std::{future::Future, pin::Pin, sync::Arc};

use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::{
    domain::provider_setup::{ProviderSetupError, ValidatedProviderCredential},
    infrastructure::providers::{BoxFuture, GpuProvider},
};

const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";
const VALIDATE_KEY_QUERY: &str = r#"
query LumaForgeValidateProviderKey {
  myself {
    id
    apiKeys {
      id
      isActive
      permissions
      isLegacy
      policies
    }
  }
}
"#;

#[derive(Clone)]
pub(crate) struct RunPodProvider {
    client: Client,
    graphql_url: Url,
    #[cfg(test)]
    transport: Option<Arc<dyn RunPodGraphqlTransport>>,
}

impl Default for RunPodProvider {
    fn default() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("RunPod reqwest client should build");
        let graphql_url = Url::parse(RUNPOD_GRAPHQL_URL).expect("RunPod GraphQL URL should parse");

        Self {
            client,
            graphql_url,
            #[cfg(test)]
            transport: None,
        }
    }
}

impl RunPodProvider {
    #[cfg(test)]
    pub(crate) fn new_with_transport(transport: impl RunPodGraphqlTransport + 'static) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("RunPod reqwest client should build");
        let graphql_url = Url::parse(RUNPOD_GRAPHQL_URL).expect("RunPod GraphQL URL should parse");

        Self {
            client,
            graphql_url,
            #[cfg(test)]
            transport: Some(Arc::new(transport)),
        }
    }

    async fn validate_api_key_inner(
        &self,
        api_key: &SecretString,
    ) -> Result<ValidatedProviderCredential, ProviderSetupError> {
        let response = self.execute_validate_key_query(api_key).await?;

        Self::validated_credential_from_response(api_key, response)
    }

    async fn execute_validate_key_query(
        &self,
        api_key: &SecretString,
    ) -> Result<RunPodGraphqlResponse, ProviderSetupError> {
        #[cfg(test)]
        if let Some(transport) = &self.transport {
            return transport.validate_key(api_key).await;
        }

        let response = self
            .client
            .post(self.graphql_url.clone())
            .bearer_auth(api_key.expose_secret())
            .json(&RunPodGraphqlRequest {
                query: VALIDATE_KEY_QUERY,
            })
            .send()
            .await
            .map_err(map_reqwest_error)?;

        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(ProviderSetupError::InvalidProviderApiKey);
            }
            status if !status.is_success() => {
                return Err(ProviderSetupError::ProviderApiUnavailable);
            }
            _ => {}
        }

        response
            .json::<RunPodGraphqlResponse>()
            .await
            .map_err(|_| ProviderSetupError::ProviderApiUnavailable)
    }

    fn validated_credential_from_response(
        api_key: &SecretString,
        response: RunPodGraphqlResponse,
    ) -> Result<ValidatedProviderCredential, ProviderSetupError> {
        if response
            .errors
            .as_ref()
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err(ProviderSetupError::InvalidProviderApiKey);
        }

        let myself = response
            .data
            .and_then(|data| data.myself)
            .ok_or(ProviderSetupError::InvalidProviderApiKey)?;

        if myself.id.trim().is_empty() {
            return Err(ProviderSetupError::InvalidProviderApiKey);
        }

        let submitted = api_key.expose_secret();
        let matching_key = myself
            .api_keys
            .into_iter()
            .find(|key| key.is_active.unwrap_or(false) && key.matches_submitted_key(submitted))
            .ok_or(ProviderSetupError::InvalidProviderApiKey)?;

        Ok(ValidatedProviderCredential {
            provider_user_id: myself.id,
            provider_api_key_fingerprint: matching_key.id,
        })
    }
}

impl GpuProvider for RunPodProvider {
    fn validate_api_key<'a>(
        &'a self,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<ValidatedProviderCredential, ProviderSetupError>> {
        Box::pin(async move { self.validate_api_key_inner(&api_key).await })
    }
}

fn map_reqwest_error(_error: reqwest::Error) -> ProviderSetupError {
    ProviderSetupError::ProviderApiUnavailable
}

#[derive(Serialize)]
struct RunPodGraphqlRequest {
    query: &'static str,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct RunPodGraphqlResponse {
    data: Option<RunPodGraphqlData>,
    errors: Option<Vec<RunPodGraphqlError>>,
}

#[derive(Clone, Debug, Deserialize)]
struct RunPodGraphqlData {
    myself: Option<RunPodUser>,
}

#[derive(Clone, Debug, Deserialize)]
struct RunPodUser {
    id: String,
    #[serde(default, rename = "apiKeys")]
    api_keys: Vec<RunPodApiKey>,
}

#[derive(Clone, Debug, Deserialize)]
struct RunPodApiKey {
    id: String,
    #[serde(default, rename = "isActive")]
    is_active: Option<bool>,
}

impl RunPodApiKey {
    fn matches_submitted_key(&self, submitted: &str) -> bool {
        !self.id.trim().is_empty() && submitted.starts_with(&self.id)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct RunPodGraphqlError {
    #[allow(dead_code)]
    message: String,
}

#[cfg(test)]
type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RunPodGraphqlResponse, ProviderSetupError>> + Send + 'a>>;

#[cfg(test)]
pub(crate) trait RunPodGraphqlTransport: Send + Sync {
    fn validate_key<'a>(&'a self, api_key: &'a SecretString) -> TransportFuture<'a>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(json: &str) -> RunPodGraphqlResponse {
        serde_json::from_str(json).expect("test response should deserialize")
    }

    #[test]
    fn derives_identity_for_active_matching_key() {
        let api_key = SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned());
        let credential = RunPodProvider::validated_credential_from_response(
            &api_key,
            response(
                r#"{
                  "data": {
                    "myself": {
                      "id": "user-123",
                      "apiKeys": [
                        {"id": "rpa_B3Q1OTCU8B", "isActive": true}
                      ]
                    }
                  }
                }"#,
            ),
        )
        .expect("active matching key should validate");

        assert_eq!(credential.provider_user_id, "user-123");
        assert_eq!(credential.provider_api_key_fingerprint, "rpa_B3Q1OTCU8B");
    }

    #[test]
    fn rejects_inactive_matching_key() {
        let api_key = SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned());
        let error = RunPodProvider::validated_credential_from_response(
            &api_key,
            response(
                r#"{
                  "data": {
                    "myself": {
                      "id": "user-123",
                      "apiKeys": [
                        {"id": "rpa_B3Q1OTCU8B", "isActive": false}
                      ]
                    }
                  }
                }"#,
            ),
        )
        .expect_err("inactive key should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
    }

    #[test]
    fn rejects_missing_matching_key() {
        let api_key = SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned());
        let error = RunPodProvider::validated_credential_from_response(
            &api_key,
            response(
                r#"{
                  "data": {
                    "myself": {
                      "id": "user-123",
                      "apiKeys": [
                        {"id": "rpa_OTHER", "isActive": true}
                      ]
                    }
                  }
                }"#,
            ),
        )
        .expect_err("missing key should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
    }

    #[test]
    fn rejects_graphql_errors() {
        let api_key = SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned());
        let error = RunPodProvider::validated_credential_from_response(
            &api_key,
            response(r#"{"errors": [{"message": "unauthorized"}]}"#),
        )
        .expect_err("graphql errors should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
    }

    #[test]
    fn rejects_malformed_identity_payload() {
        let api_key = SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned());
        let error = RunPodProvider::validated_credential_from_response(
            &api_key,
            response(r#"{"data": {"myself": null}}"#),
        )
        .expect_err("missing myself should fail");

        assert!(matches!(error, ProviderSetupError::InvalidProviderApiKey));
    }

    struct UnavailableTransport;

    impl RunPodGraphqlTransport for UnavailableTransport {
        fn validate_key<'a>(&'a self, _api_key: &'a SecretString) -> TransportFuture<'a> {
            Box::pin(async { Err(ProviderSetupError::ProviderApiUnavailable) })
        }
    }

    #[tokio::test]
    async fn maps_transport_timeout_to_provider_api_unavailable() {
        let provider = RunPodProvider::new_with_transport(UnavailableTransport);
        let error = provider
            .validate_api_key(SecretString::from("rpa_B3Q1OTCU8B_secret_tail".to_owned()))
            .await
            .expect_err("provider transport failure should fail");

        assert!(matches!(error, ProviderSetupError::ProviderApiUnavailable));
    }
}
