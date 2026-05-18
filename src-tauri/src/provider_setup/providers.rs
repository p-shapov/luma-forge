use crate::{
    domain::provider_setup::{GpuCloudProviderId, ProviderApiKey, ProviderIdentity},
    provider::{runpod::RunPodClient, ProviderClientError},
};

use super::ProviderSetupError;

pub(crate) async fn validate_identity(
    provider_id: &GpuCloudProviderId,
    api_key: &ProviderApiKey,
) -> Result<ProviderIdentity, ProviderSetupError> {
    match provider_id {
        GpuCloudProviderId::Runpod => runpod::validate_identity(api_key).await,
    }
}

pub(in crate::provider_setup) fn provider_setup_error_from_client_error(
    error: ProviderClientError,
) -> ProviderSetupError {
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

pub(in crate::provider_setup) mod runpod {
    use super::*;

    pub(super) async fn validate_identity(
        api_key: &ProviderApiKey,
    ) -> Result<ProviderIdentity, ProviderSetupError> {
        validate_identity_with_client(api_key, &RunPodClient::default()).await
    }

    async fn validate_identity_with_client(
        api_key: &ProviderApiKey,
        client: &RunPodClient,
    ) -> Result<ProviderIdentity, ProviderSetupError> {
        client
            .validate_identity(api_key)
            .await
            .map_err(provider_setup_error_from_client_error)
    }
}
