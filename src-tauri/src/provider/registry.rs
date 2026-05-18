use std::{future::Future, pin::Pin};

use crate::{
    domain::{
        placement::ProviderPlacementCapabilities,
        provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    },
    provider::{error::ProviderClientError, runpod::RunPodClient},
    provider_setup::{ProviderIdentityGateway, ProviderSetupError},
    secrets::{KeyringSecretStore, SecretStore},
    workspace_setup::{
        contracts::ProviderPlacementOptions, error::WorkspaceSetupError,
        ProviderPlacementOptionsGateway,
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

impl<S> ProviderPlacementOptionsGateway for ProviderClientRegistry<S>
where
    S: SecretStore,
{
    fn fetch_placement_options<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let api_key = self
                .secrets
                .read_api_key(provider_id)?
                .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

            let provider_inventory = match provider_id {
                GpuCloudProviderId::Runpod => self
                    .runpod
                    .fetch_inventory(&api_key)
                    .await
                    .map_err(error_from_client_error),
            }?;
            let placement_capabilities = ProviderPlacementCapabilities::for_provider(*provider_id);

            Ok(ProviderPlacementOptions {
                provider_inventory,
                placement_capabilities,
            })
        })
    }
}

fn provider_setup_error_from_client_error(error: ProviderClientError) -> ProviderSetupError {
    match error {
        ProviderClientError::Unauthorized => ProviderSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable | ProviderClientError::RateLimited => {
            ProviderSetupError::ProviderApiUnavailable
        }
        ProviderClientError::RequestRejected
        | ProviderClientError::ResponseInvalid
        | ProviderClientError::NotFound
        | ProviderClientError::Conflict
        | ProviderClientError::Indeterminate => ProviderSetupError::ProviderIdentityResponseInvalid,
    }
}

fn error_from_client_error(error: ProviderClientError) -> WorkspaceSetupError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => WorkspaceSetupError::ProviderApiUnavailable,
        ProviderClientError::RateLimited => WorkspaceSetupError::ProviderRateLimited,
        ProviderClientError::RequestRejected => WorkspaceSetupError::ProviderRequestRejected,
        ProviderClientError::ResponseInvalid
        | ProviderClientError::NotFound
        | ProviderClientError::Conflict
        | ProviderClientError::Indeterminate => WorkspaceSetupError::ProviderResponseInvalid,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
