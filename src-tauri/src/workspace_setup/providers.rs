use crate::{
    domain::{placement::ProviderPlacementCapabilities, provider_setup::GpuCloudProviderId},
    provider::ProviderClientError,
    secrets::SecretStore,
};

use super::{contracts::ProviderPlacementOptions, error::WorkspaceSetupError};

pub(crate) async fn fetch_placement_options(
    secrets: &impl SecretStore,
    provider_id: &GpuCloudProviderId,
) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
    let api_key = secrets
        .read_api_key(provider_id)?
        .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

    let provider_inventory = match provider_id {
        GpuCloudProviderId::Runpod => runpod::fetch_inventory(&api_key).await,
    }?;
    let placement_capabilities = ProviderPlacementCapabilities::for_provider(*provider_id);

    Ok(ProviderPlacementOptions {
        provider_inventory,
        placement_capabilities,
    })
}

fn workspace_setup_error_from_client_error(error: ProviderClientError) -> WorkspaceSetupError {
    match error {
        ProviderClientError::Unauthorized => WorkspaceSetupError::ProviderApiKeyUnauthorized,
        ProviderClientError::ApiUnavailable => WorkspaceSetupError::ProviderApiUnavailable,
        ProviderClientError::RateLimited => WorkspaceSetupError::ProviderRateLimited,
        ProviderClientError::RequestRejected => WorkspaceSetupError::ProviderRequestRejected,
        ProviderClientError::ResponseInvalid
        | ProviderClientError::NotFound
        | ProviderClientError::Conflict
        | ProviderClientError::Indeterminate => WorkspaceSetupError::ProviderResponseInvalid,
    }
}

mod runpod {
    use crate::{
        domain::{provider_inventory::ProviderInventory, provider_setup::ProviderApiKey},
        provider::runpod::RunPodClient,
    };

    use super::{workspace_setup_error_from_client_error, WorkspaceSetupError};

    pub(super) async fn fetch_inventory(
        api_key: &ProviderApiKey,
    ) -> Result<ProviderInventory, WorkspaceSetupError> {
        fetch_inventory_with_client(api_key, &RunPodClient::default()).await
    }

    async fn fetch_inventory_with_client(
        api_key: &ProviderApiKey,
        client: &RunPodClient,
    ) -> Result<ProviderInventory, WorkspaceSetupError> {
        client
            .fetch_inventory(api_key)
            .await
            .map_err(workspace_setup_error_from_client_error)
    }

    #[cfg(test)]
    pub(super) async fn fetch_inventory_for_test(
        api_key: &ProviderApiKey,
        client: &RunPodClient,
    ) -> Result<ProviderInventory, WorkspaceSetupError> {
        fetch_inventory_with_client(api_key, client).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        domain::provider_setup::ProviderApiKey,
        provider::runpod::RunPodClient,
        secrets::{ProvisionerWorkerBearerToken, SecretStoreError},
    };

    use super::*;

    #[derive(Debug, Clone, Default)]
    struct EmptySecretStore;

    impl SecretStore for EmptySecretStore {
        fn has_api_key_entry(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<bool, SecretStoreError> {
            Ok(false)
        }

        fn read_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
            Ok(None)
        }

        fn replace_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
            _api_key: &ProviderApiKey,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace setup provider tests do not write secrets")
        }

        fn delete_api_key(
            &self,
            _provider_id: &GpuCloudProviderId,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace setup provider tests do not delete secrets")
        }

        fn write_provisioner_worker_token(
            &self,
            _workspace_id: &str,
            _token: &ProvisionerWorkerBearerToken,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace setup provider tests do not write provisioner tokens")
        }

        fn read_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
            unimplemented!("workspace setup provider tests do not read provisioner tokens")
        }

        fn delete_provisioner_worker_token(
            &self,
            _workspace_id: &str,
        ) -> Result<(), SecretStoreError> {
            unimplemented!("workspace setup provider tests do not delete provisioner tokens")
        }
    }

    #[tokio::test]
    async fn inventory_reads_api_key_from_secret_store() {
        let error = fetch_placement_options(&EmptySecretStore, &GpuCloudProviderId::Runpod)
            .await
            .expect_err("missing key should fail before provider call");

        assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
    }

    #[tokio::test]
    async fn inventory_dispatches_to_runpod_with_stored_key() {
        let runpod =
            RunPodClient::new_for_test("http://127.0.0.1:9".to_string(), Duration::from_millis(50));
        let api_key = ProviderApiKey::new("rp_123_secret".to_string()).expect("valid api key");

        let error = runpod::fetch_inventory_for_test(&api_key, &runpod)
            .await
            .expect_err("unreachable inventory endpoint should fail");

        assert_eq!(error, WorkspaceSetupError::ProviderApiUnavailable);
    }

    #[test]
    fn inventory_auth_failure_maps_to_provider_key_unauthorized() {
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::Unauthorized),
            WorkspaceSetupError::ProviderApiKeyUnauthorized
        );
    }

    #[test]
    fn inventory_rate_limit_maps_to_retryable_provider_availability() {
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::RateLimited),
            WorkspaceSetupError::ProviderRateLimited
        );
    }

    #[test]
    fn inventory_request_rejection_does_not_collapse_to_unavailable() {
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::RequestRejected),
            WorkspaceSetupError::ProviderRequestRejected
        );
    }

    #[test]
    fn invalid_provider_response_maps_to_provider_response_invalid() {
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::ResponseInvalid),
            WorkspaceSetupError::ProviderResponseInvalid
        );
    }
}
