use std::{future::Future, pin::Pin, sync::Arc};

use secrecy::SecretString;

use crate::domain::provider_setup::{
    GpuCloudProviderId, ProviderSetupError, ValidatedProviderCredential,
};

mod runpod;

pub(crate) use runpod::RunPodProvider;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub(crate) trait GpuProvider: Send + Sync {
    fn validate_api_key<'a>(
        &'a self,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<ValidatedProviderCredential, ProviderSetupError>>;
}

#[derive(Clone)]
pub(crate) struct SharedGpuProvider(Arc<dyn GpuProvider>);

impl SharedGpuProvider {
    fn new(provider: impl GpuProvider + 'static) -> Self {
        Self(Arc::new(provider))
    }
}

impl GpuProvider for SharedGpuProvider {
    fn validate_api_key<'a>(
        &'a self,
        api_key: SecretString,
    ) -> BoxFuture<'a, Result<ValidatedProviderCredential, ProviderSetupError>> {
        self.0.validate_api_key(api_key)
    }
}

#[derive(Clone)]
pub(crate) struct GpuProviderRegistry {
    runpod: SharedGpuProvider,
}

impl GpuProviderRegistry {
    pub(crate) fn new(runpod: impl GpuProvider + 'static) -> Self {
        Self {
            runpod: SharedGpuProvider::new(runpod),
        }
    }

    pub(crate) fn provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> Result<SharedGpuProvider, ProviderSetupError> {
        match provider_id {
            GpuCloudProviderId::RunPod => Ok(self.runpod.clone()),
        }
    }
}
