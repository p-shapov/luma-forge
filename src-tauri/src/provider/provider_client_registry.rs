use std::{future::Future, pin::Pin};

use crate::{
    domain::{
        provider_inventory::ProviderInventory,
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    },
    provider::{provider_client_error::ProviderClientError, runpod::RunPodClient},
    provider_setup::{ProviderIdentityGateway, ProviderSetupError},
    secrets::{KeyringSecretStore, SecretStore},
    workspace_setup::{
        workspace_setup_error::WorkspaceSetupError,
        workspace_setup_service::ProviderInventoryGateway,
    },
};

#[derive(Debug, Clone)]
pub struct ProviderClientRegistry<S = KeyringSecretStore> {
    secrets: S,
    runpod: RunPodClient,
}

impl<S> ProviderClientRegistry<S> {
    pub fn new(secrets: S, runpod: RunPodClient) -> Self {
        Self { secrets, runpod }
    }
}

impl Default for ProviderClientRegistry<KeyringSecretStore> {
    fn default() -> Self {
        Self::new(KeyringSecretStore, RunPodClient::default())
    }
}

impl<S> ProviderIdentityGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .validate_identity(api_key)
                    .await
                    .map_err(provider_setup_error_from_client_error),
            }
        })
    }
}

impl<S> ProviderInventoryGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn fetch_inventory<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInventory, WorkspaceSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            let api_key = self
                .secrets
                .read_api_key(provider_id)?
                .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

            match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .fetch_inventory(&api_key)
                    .await
                    .map_err(workspace_setup_error_from_client_error),
            }
        })
    }
}

fn provider_setup_error_from_client_error(error: ProviderClientError) -> ProviderSetupError {
    match error {
        ProviderClientError::Unauthorized => ProviderSetupError::InvalidProviderApiKey,
        ProviderClientError::ApiUnavailable => ProviderSetupError::ProviderApiUnavailable,
        ProviderClientError::IdentityUnavailable => ProviderSetupError::ProviderIdentityUnavailable,
    }
}

fn workspace_setup_error_from_client_error(error: ProviderClientError) -> WorkspaceSetupError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceSetupError::InvalidProviderApiKey,
        ProviderClientError::ApiUnavailable | ProviderClientError::IdentityUnavailable => {
            WorkspaceSetupError::ProviderApiUnavailable
        }
    }
}

#[cfg(test)]
#[path = "provider_client_tests.rs"]
mod provider_client_tests;
