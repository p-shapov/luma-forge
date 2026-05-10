use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey},
    provider::{error::ProviderClientError, runpod::RunPodClient, ProviderClientRegistry},
    secrets::{SecretStore, SecretStoreError},
    workspace_setup::{error::WorkspaceSetupError, ProviderInventoryGateway},
};

use super::error_from_client_error;

#[derive(Debug, Clone, Default)]
struct EmptySecretStore;

impl SecretStore for EmptySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(false)
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        Ok(None)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not write secrets")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not delete secrets")
    }
}

#[tokio::test]
async fn inventory_reads_api_key_from_secret_store() {
    let registry = ProviderClientRegistry::new(EmptySecretStore, RunPodClient::default());

    let error = registry
        .fetch_inventory(&GpuCloudProviderId::Runpod)
        .await
        .expect_err("missing key should fail before provider call");

    assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
}

#[test]
fn inventory_auth_failure_maps_to_provider_key_unauthorized() {
    assert_eq!(
        error_from_client_error(ProviderClientError::Unauthorized),
        WorkspaceSetupError::ProviderApiKeyUnauthorized
    );
}
