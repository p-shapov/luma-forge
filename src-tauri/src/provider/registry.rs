use std::{future::Future, pin::Pin};

use crate::{
    domain::{
        provider_inventory::ProviderInventory,
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    },
    provider::{error::ProviderClientError, runpod::RunPodClient},
    provider_setup::{ProviderIdentityGateway, ProviderSetupError},
    secrets::{KeyringSecretStore, SecretStore},
    workspace_setup::{error::WorkspaceSetupError, ProviderInventoryGateway},
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
                    .map_err(error_from_client_error),
            }
        })
    }
}

fn provider_setup_error_from_client_error(error: ProviderClientError) -> ProviderSetupError {
    match error {
        ProviderClientError::Unauthorized => ProviderSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => ProviderSetupError::ProviderApiUnavailable,
        ProviderClientError::ResponseInvalid => ProviderSetupError::ProviderIdentityResponseInvalid,
    }
}

fn error_from_client_error(error: ProviderClientError) -> WorkspaceSetupError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => WorkspaceSetupError::ProviderApiUnavailable,
        ProviderClientError::ResponseInvalid => WorkspaceSetupError::ProviderResponseInvalid,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
