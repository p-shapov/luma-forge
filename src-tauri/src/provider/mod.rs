use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    provider_setup::{ProviderIdentityGateway, ProviderSetupError},
};

pub mod runpod;

#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    runpod: runpod::RunPodClient,
}

impl ProviderRegistry {
    pub fn new(runpod: runpod::RunPodClient) -> Self {
        Self { runpod }
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new(runpod::RunPodClient::default())
    }
}

impl ProviderIdentityGateway for ProviderRegistry {
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>
    {
        Box::pin(async move {
            match provider_id {
                GpuCloudProviderId::Runpod => self.runpod.validate_identity(api_key).await,
            }
        })
    }
}
