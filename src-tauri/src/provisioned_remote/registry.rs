use std::sync::Arc;

use crate::domain::provider::GpuCloudProviderId;

use super::{errors::ProvisionedRemoteError, provider::ProvisionedRemoteProvider};

#[derive(Clone)]
pub struct ProvisionedRemoteProviderRegistry {
    providers: Vec<Arc<dyn ProvisionedRemoteProvider>>,
}

impl ProvisionedRemoteProviderRegistry {
    pub fn new(providers: Vec<Box<dyn ProvisionedRemoteProvider>>) -> Self {
        Self {
            providers: providers.into_iter().map(Arc::from).collect(),
        }
    }

    pub fn with_provider(provider: Box<dyn ProvisionedRemoteProvider>) -> Self {
        Self {
            providers: vec![Arc::from(provider)],
        }
    }

    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn for_provider(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<&dyn ProvisionedRemoteProvider, ProvisionedRemoteError> {
        self.providers
            .iter()
            .find(|provider| provider.provider_id() == provider_id)
            .map(|provider| provider.as_ref())
            .ok_or(ProvisionedRemoteError::ProviderAdapterUnavailable)
    }
}
