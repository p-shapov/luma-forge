use std::sync::Arc;

use super::{errors::ProvisionedRemoteError, provider::RunpodRuntimeClient};

#[derive(Clone)]
pub struct ProvisionedRemoteProviderRegistry {
    providers: Vec<Arc<dyn RunpodRuntimeClient>>,
}

impl ProvisionedRemoteProviderRegistry {
    pub fn new(providers: Vec<Box<dyn RunpodRuntimeClient>>) -> Self {
        Self {
            providers: providers.into_iter().map(Arc::from).collect(),
        }
    }

    pub fn with_provider(provider: Box<dyn RunpodRuntimeClient>) -> Self {
        Self {
            providers: vec![Arc::from(provider)],
        }
    }

    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn for_provider(&self) -> Result<&dyn RunpodRuntimeClient, ProvisionedRemoteError> {
        self.providers
            .first()
            .map(|provider| provider.as_ref())
            .ok_or(ProvisionedRemoteError::ProviderAdapterUnavailable)
    }
}
