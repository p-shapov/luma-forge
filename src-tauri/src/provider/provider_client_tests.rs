use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey},
    provider::{provider_client_registry::ProviderClientRegistry, runpod::RunPodClient},
    provider_setup::ProviderSetupError,
    secrets::SecretStore,
    workspace::{
        workspace_setup_error::WorkspaceSetupError,
        workspace_setup_service::ProviderInventoryGateway,
    },
};

#[derive(Debug, Clone, Default)]
struct EmptySecretStore;

impl SecretStore for EmptySecretStore {
    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, ProviderSetupError> {
        Ok(None)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), ProviderSetupError> {
        unimplemented!("provider registry tests do not write secrets")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), ProviderSetupError> {
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
