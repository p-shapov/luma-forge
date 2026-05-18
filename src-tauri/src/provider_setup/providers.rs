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

mod runpod {
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

    #[cfg(test)]
    pub(super) async fn validate_identity_for_test(
        api_key: &ProviderApiKey,
        client: &RunPodClient,
    ) -> Result<ProviderIdentity, ProviderSetupError> {
        validate_identity_with_client(api_key, client).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::domain::provider_setup::ProviderApiKey;

    use super::*;

    #[test]
    fn auth_failure_maps_to_provider_key_unauthorized() {
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::Unauthorized),
            ProviderSetupError::ProviderApiKeyUnauthorized
        );
    }

    #[test]
    fn rate_limit_maps_to_provider_api_unavailable() {
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::RateLimited),
            ProviderSetupError::ProviderApiUnavailable
        );
    }

    #[test]
    fn request_rejection_maps_to_identity_response_invalid() {
        assert_eq!(
            provider_setup_error_from_client_error(ProviderClientError::RequestRejected),
            ProviderSetupError::ProviderIdentityResponseInvalid
        );
    }

    #[tokio::test]
    async fn dispatches_runpod_identity_validation() {
        let runpod =
            RunPodClient::new_for_test("http://127.0.0.1:9".to_string(), Duration::from_millis(50));
        let api_key = ProviderApiKey::new("rp_123_secret".to_string()).expect("valid api key");

        let error = runpod::validate_identity_for_test(&api_key, &runpod)
            .await
            .expect_err("unreachable provider should fail");

        assert_eq!(error, ProviderSetupError::ProviderApiUnavailable);
    }
}
