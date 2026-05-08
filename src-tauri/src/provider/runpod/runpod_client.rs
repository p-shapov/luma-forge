use crate::{
    domain::{provider_inventory::ProviderInventory, provider_setup::ProviderApiKey},
    provider::{
        provider_client_error::ProviderClientError,
        runpod::{
            runpod_contracts::{
                GraphQlRequest, GraphQlResponse, RunPodIdentityData, RunPodInventoryData,
            },
            runpod_mapper::{identity_from_graphql_response, inventory_from_graphql_response},
        },
    },
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
const INVENTORY_QUERY: &str = r#"
query LumaForgeProviderInventory {
  dataCenters {
    id
    name
    gpuAvailability {
      stockStatus
      gpuType {
        id
        displayName
        memoryInGb
      }
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
    ) -> Result<crate::domain::provider_setup::ProviderIdentity, ProviderClientError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphQlRequest {
                query: IDENTITY_QUERY,
            })
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderClientError::Unauthorized);
        }
        if !status.is_success() {
            return Err(ProviderClientError::ApiUnavailable);
        }

        let payload = response
            .json::<GraphQlResponse<RunPodIdentityData>>()
            .await
            .map_err(|_| ProviderClientError::IdentityUnavailable)?;

        identity_from_graphql_response(api_key, payload)
    }

    pub async fn fetch_inventory(
        &self,
        api_key: &ProviderApiKey,
    ) -> Result<ProviderInventory, ProviderClientError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key.expose_secret())
            .json(&GraphQlRequest {
                query: INVENTORY_QUERY,
            })
            .send()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        if !response.status().is_success() {
            return Err(ProviderClientError::ApiUnavailable);
        }

        let payload = response
            .json::<GraphQlResponse<RunPodInventoryData>>()
            .await
            .map_err(|_| ProviderClientError::ApiUnavailable)?;

        inventory_from_graphql_response(payload)
    }
}
