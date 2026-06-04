use crate::{
    domain::{provider::GpuCloudProviderId, shared::ApiKeySetup},
    shared::AppFuture,
};

use super::{error::ProviderApiKeyError, store::ProviderApiKey};

pub trait ProviderIdentityValidator: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: GpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> AppFuture<'a, Result<ApiKeySetup, ProviderApiKeyError>>;
}
