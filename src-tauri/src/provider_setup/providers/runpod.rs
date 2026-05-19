use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{ProviderApiKey, ProviderIdentity},
    provider::runpod::RunPodClient,
};

use super::{provider_setup_error_from_client_error, ProviderSetupCapability, ProviderSetupError};

#[derive(Debug, Clone, Default)]
pub(super) struct RunPodProviderSetupService {
    client: RunPodClient,
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
