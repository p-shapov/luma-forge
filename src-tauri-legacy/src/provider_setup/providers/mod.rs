mod runpod;

use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    provider::{runpod::RunPodHttpClientInitError, ProviderClientError},
};

use super::ProviderSetupError;

pub trait ProviderSetupCapability: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>;
}

pub trait ProviderSetupProviderResolver: Send + Sync {
    fn for_provider(&self, provider_id: &GpuCloudProviderId) -> &dyn ProviderSetupCapability;
}

#[derive(Debug, Clone)]
pub struct ProviderSetupProviderRegistry {
    runpod: runpod::RunPodProviderSetupService,
}

impl ProviderSetupProviderRegistry {
    pub fn try_new() -> Result<Self, RunPodHttpClientInitError> {
        Ok(Self {
            runpod: runpod::RunPodProviderSetupService::try_new()?,
        })
    }
}

impl ProviderSetupProviderResolver for ProviderSetupProviderRegistry {
    fn for_provider(&self, provider_id: &GpuCloudProviderId) -> &dyn ProviderSetupCapability {
        match provider_id {
            GpuCloudProviderId::Runpod => &self.runpod,
        }
    }
}

pub(in crate::provider_setup) fn provider_setup_error_from_client_error(
    error: ProviderClientError,
) -> ProviderSetupError {
    match error {
        ProviderClientError::Unauthorized | ProviderClientError::InsufficientPermissions => {
            ProviderSetupError::ProviderApiKeyUnauthorized
        }
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

#[cfg(test)]
fn provider_setup_error_from_runpod_init_error(
    _error: RunPodHttpClientInitError,
) -> ProviderSetupError {
    ProviderSetupError::ProviderApiUnavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_selects_runpod_capability_for_runpod_provider() {
        let registry = ProviderSetupProviderRegistry::try_new().expect("registry initializes");

        let capability = registry.for_provider(&GpuCloudProviderId::Runpod);

        assert!(std::ptr::addr_eq(
            capability as *const dyn ProviderSetupCapability,
            &registry.runpod as &dyn ProviderSetupCapability as *const dyn ProviderSetupCapability,
        ));
    }

    #[test]
    fn maps_provider_client_errors_to_provider_setup_errors() {
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::Unauthorized),
            ProviderSetupError::ProviderApiKeyUnauthorized
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::ApiUnavailable),
            ProviderSetupError::ProviderApiUnavailable
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::RateLimited),
            ProviderSetupError::ProviderApiUnavailable
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::RequestRejected),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::ResponseInvalid),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::NotFound),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::Conflict),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::Indeterminate),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
    }

    #[test]
    fn maps_runpod_http_initialization_errors_to_provider_unavailable() {
        assert_eq!(
            provider_setup_error_from_runpod_init_error(
                crate::provider::runpod::RunPodHttpClientInitError,
            ),
            ProviderSetupError::ProviderApiUnavailable
        );
    }
}
