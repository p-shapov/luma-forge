use std::{collections::HashMap, time::Duration};

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::ProviderResourceStatus,
    },
    provider::{
        error::ProviderClientError,
        runpod::{RunPodClient, RunPodTemplateObservation},
        ProviderClientRegistry,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStore, SecretStoreError},
    workspace_provisioning::{
        CreateNetworkVolumeInput, ProviderProvisioningGateway, WorkspaceProvisioningError,
    },
    workspace_setup::{error::WorkspaceSetupError, ProviderPlacementOptionsGateway},
};

use super::{
    error_from_client_error, provisioning_error_from_client_error, runpod_template_observation,
};

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
        unimplemented!("provider registry tests do not write secrets")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not delete secrets")
    }

    fn write_provisioner_worker_token(
        &self,
        _workspace_id: &str,
        _token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not write provisioner tokens")
    }

    fn read_provisioner_worker_token(
        &self,
        _workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        unimplemented!("provider registry tests do not read provisioner tokens")
    }

    fn delete_provisioner_worker_token(&self, _workspace_id: &str) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not delete provisioner tokens")
    }
}

#[derive(Debug, Clone)]
struct ApiKeySecretStore {
    api_key: String,
}

impl SecretStore for ApiKeySecretStore {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        Ok(true)
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        ProviderApiKey::new(self.api_key.clone())
            .map(Some)
            .map_err(|_| SecretStoreError::InvalidStoredProviderApiKey)
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not write secrets")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not delete secrets")
    }

    fn write_provisioner_worker_token(
        &self,
        _workspace_id: &str,
        _token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not write provisioner tokens")
    }

    fn read_provisioner_worker_token(
        &self,
        _workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        unimplemented!("provider registry tests do not read provisioner tokens")
    }

    fn delete_provisioner_worker_token(&self, _workspace_id: &str) -> Result<(), SecretStoreError> {
        unimplemented!("provider registry tests do not delete provisioner tokens")
    }
}

#[tokio::test]
async fn inventory_reads_api_key_from_secret_store() {
    let registry = ProviderClientRegistry::new(EmptySecretStore, RunPodClient::default());

    let error = registry
        .fetch_placement_options(&GpuCloudProviderId::Runpod)
        .await
        .expect_err("missing key should fail before provider call");

    assert_eq!(error, WorkspaceSetupError::ProviderSetupIncomplete);
}

#[test]
fn inventory_auth_failure_maps_to_provider_key_unauthorized() {
    assert_eq!(
        error_from_client_error(ProviderClientError::Unauthorized),
        WorkspaceSetupError::ProviderApiKeyUnauthorized
    );
}

#[test]
fn inventory_rate_limit_maps_to_retryable_provider_availability() {
    assert_eq!(
        error_from_client_error(ProviderClientError::RateLimited),
        WorkspaceSetupError::ProviderRateLimited
    );
}

#[test]
fn inventory_request_rejection_does_not_collapse_to_unavailable() {
    assert_eq!(
        error_from_client_error(ProviderClientError::RequestRejected),
        WorkspaceSetupError::ProviderRequestRejected
    );
}

#[test]
fn provisioning_request_rejection_maps_to_request_rejected() {
    assert_eq!(
        provisioning_error_from_client_error(ProviderClientError::RequestRejected),
        WorkspaceProvisioningError::ProviderRequestRejected
    );
}

#[test]
fn provisioning_rate_limit_maps_to_rate_limited() {
    assert_eq!(
        provisioning_error_from_client_error(ProviderClientError::RateLimited),
        WorkspaceProvisioningError::ProviderRateLimited
    );
}

#[test]
fn runpod_template_observation_preserves_template_env() {
    let env = HashMap::from([("CUSTOM_ENV".to_string(), "custom-value".to_string())]);

    let observation = runpod_template_observation(RunPodTemplateObservation {
        id: "template-1".to_string(),
        image_name: "ghcr.io/luma-forge/endpoint-worker:dev".to_string(),
        volume_mount_path: "/workspace".to_string(),
        env: env.clone(),
        status: ProviderResourceStatus::Ready,
    });

    assert_eq!(observation.runtime_env, env);
}

#[tokio::test]
async fn provisioning_dispatch_reads_stored_key_before_runpod_call() {
    let registry = ProviderClientRegistry::new(
        ApiKeySecretStore {
            api_key: "rp_123_secret".to_string(),
        },
        RunPodClient::new_for_test("http://127.0.0.1:9".to_string(), Duration::from_millis(50)),
    );

    let error = registry
        .create_network_volume(CreateNetworkVolumeInput {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            datacenter_id: "EU-RO-1".to_string(),
            size_bytes: 80 * 1024 * 1024 * 1024,
        })
        .await
        .expect_err("unreachable create should be indeterminate after dispatch");

    assert_eq!(
        error,
        WorkspaceProvisioningError::ProviderOperationIndeterminate
    );
}

#[tokio::test]
async fn provisioning_fails_before_provider_call_when_setup_missing() {
    let registry = ProviderClientRegistry::new(
        EmptySecretStore,
        RunPodClient::new_for_test("http://127.0.0.1:9".to_string(), Duration::from_millis(50)),
    );

    let error = registry
        .create_network_volume(CreateNetworkVolumeInput {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            workspace_id: "018f6a40-0000-7000-8000-000000000001".to_string(),
            datacenter_id: "EU-RO-1".to_string(),
            size_bytes: 80 * 1024 * 1024 * 1024,
        })
        .await
        .expect_err("missing setup should fail before provider call");

    assert_eq!(error, WorkspaceProvisioningError::ProviderSetupIncomplete);
}

#[tokio::test]
async fn provisioning_maps_runpod_transport_failure_to_workspace_provisioning_error() {
    let registry = ProviderClientRegistry::new(
        ApiKeySecretStore {
            api_key: "rp_123_secret".to_string(),
        },
        RunPodClient::new_for_test("http://127.0.0.1:9".to_string(), Duration::from_millis(50)),
    );

    let error = registry
        .get_network_volume(GpuCloudProviderId::Runpod, "missing-volume")
        .await
        .expect_err("unreachable get should map");

    assert_eq!(error, WorkspaceProvisioningError::ProviderApiUnavailable);
}
