use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{ProviderApiKey, ProviderIdentity},
    provider::runpod::{RunPodClient, RunPodHttpClientInitError},
};

use super::{provider_setup_error_from_client_error, ProviderSetupCapability, ProviderSetupError};

#[derive(Debug, Clone)]
pub(super) struct RunPodProviderSetupService {
    client: RunPodClient,
}

impl RunPodProviderSetupService {
    pub(super) fn new(client: RunPodClient) -> Self {
        Self { client }
    }

    pub(super) fn try_new() -> Result<Self, RunPodHttpClientInitError> {
        Ok(Self::new(RunPodClient::try_new_default()?))
    }
}

impl ProviderSetupCapability for RunPodProviderSetupService {
    fn validate_identity<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            self.client
                .validate_identity(api_key)
                .await
                .map_err(provider_setup_error_from_client_error)
        })
    }
}
