use super::*;

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    domain::{
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
        workspace::{
            PersistentStorageVolumeSnapshot, ProviderResourceStatus, ProvisioningPodSnapshot,
            RunPodEndpointTemplateSnapshot, ServerlessEndpointSnapshot, Workspace,
        },
    },
    provider_resources::{
        CreateEndpointTemplateInput, CreateNetworkVolumeInput, CreateProvisioningPodInput,
        CreateServerlessEndpointInput, DiscoverEndpointTemplatesInput, DiscoverNetworkVolumesInput,
        DiscoverProvisioningPodsInput, DiscoverServerlessEndpointsInput,
        EndpointTemplateObservation, NetworkVolumeObservation, ObserveProvisioningPodInput,
        ProvisioningPodObservation, ServerlessEndpointObservation,
    },
    secrets::{ProvisionerWorkerBearerToken, SecretStoreError},
    workspace_setup::tests::sample_workspace,
};

#[tokio::test]
async fn deletes_known_resources_in_cleanup_order_and_deletes_worker_token() {
    let workspace = workspace_with_resources();
    let provider = CleanupProvider::default();
    let secrets = CleanupSecrets::default();

    cleanup_known_resources(&secrets, &provider, &workspace)
        .await
        .expect("cleanup should succeed");

    assert_eq!(
        provider.calls(),
        vec![
            "endpoint:endpoint-1",
            "template:template-1",
            "pod:pod-1",
            "volume:volume-1"
        ]
    );
    assert_eq!(secrets.deleted_tokens(), vec![workspace.id]);
}

#[tokio::test]
async fn tolerates_provider_resources_that_are_already_missing() {
    let workspace = workspace_with_resources();
    let provider = CleanupProvider {
        endpoint_delete_error: Some(ProviderResourceError::ProviderResourceNotFound),
        template_delete_error: Some(ProviderResourceError::ProviderResourceNotFound),
        pod_delete_error: Some(ProviderResourceError::ProviderResourceNotFound),
        volume_delete_error: Some(ProviderResourceError::ProviderResourceNotFound),
        ..Default::default()
    };
    let secrets = CleanupSecrets::default();

    cleanup_known_resources(&secrets, &provider, &workspace)
        .await
        .expect("missing provider resources should be tolerated");

    assert_eq!(provider.calls().len(), 4);
    assert_eq!(secrets.deleted_tokens(), vec![workspace.id]);
}

#[tokio::test]
async fn returns_first_error_after_attempting_remaining_cleanup() {
    let workspace = workspace_with_resources();
    let provider = CleanupProvider {
        endpoint_delete_error: Some(ProviderResourceError::ProviderApiUnavailable),
        template_delete_error: Some(ProviderResourceError::ProviderRateLimited),
        ..Default::default()
    };
    let secrets = CleanupSecrets {
        delete_error: Some(SecretStoreError::SecureKeyringUnavailable),
        ..Default::default()
    };

    let error = cleanup_known_resources(&secrets, &provider, &workspace)
        .await
        .expect_err("cleanup should return the first failure");

    assert_eq!(error, WorkspaceProvisioningError::ProviderApiUnavailable);
    assert_eq!(
        provider.calls(),
        vec![
            "endpoint:endpoint-1",
            "template:template-1",
            "pod:pod-1",
            "volume:volume-1"
        ]
    );
    assert_eq!(secrets.deleted_tokens(), vec![workspace.id]);
}

#[tokio::test]
async fn skips_absent_resource_snapshots_and_still_deletes_worker_token() {
    let workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    let provider = CleanupProvider::default();
    let secrets = CleanupSecrets::default();

    cleanup_known_resources(&secrets, &provider, &workspace)
        .await
        .expect("cleanup should succeed");

    assert!(provider.calls().is_empty());
    assert_eq!(secrets.deleted_tokens(), vec![workspace.id]);
}

fn workspace_with_resources() -> Workspace {
    let mut workspace = sample_workspace("018f6a40-0000-7000-8000-000000000001");
    workspace.persistent_storage_volume_snapshot = Some(PersistentStorageVolumeSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "volume-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        mount_path: "/workspace".to_string(),
    });
    workspace.active_provisioning_pod_snapshot = Some(ProvisioningPodSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "pod-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Running,
        provisioner_status_url: "https://pod.example/status".to_string(),
    });
    workspace.provider_provisioning_snapshot = Some(ProviderProvisioningSnapshot::Runpod {
        endpoint_template_snapshot: Some(RunPodEndpointTemplateSnapshot {
            template_id: "template-1".to_string(),
            provider_resource_status: ProviderResourceStatus::Ready,
            endpoint_worker_image_ref: "registry.example/endpoint:latest".to_string(),
            mount_path: "/workspace".to_string(),
        }),
    });
    workspace.serverless_endpoint_snapshot = Some(ServerlessEndpointSnapshot {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_resource_id: "endpoint-1".to_string(),
        provider_resource_status: ProviderResourceStatus::Ready,
        endpoint_invoke_url: "https://endpoint.example/run".to_string(),
    });
    workspace
}

#[derive(Debug, Clone, Default)]
struct CleanupSecrets {
    deleted_tokens: Arc<Mutex<Vec<String>>>,
    delete_error: Option<SecretStoreError>,
}

impl CleanupSecrets {
    fn deleted_tokens(&self) -> Vec<String> {
        self.deleted_tokens
            .lock()
            .expect("deleted token lock")
            .clone()
    }
}

impl SecretStore for CleanupSecrets {
    fn has_api_key_entry(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<bool, SecretStoreError> {
        unimplemented!("cleanup tests do not read provider api keys")
    }

    fn read_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
    ) -> Result<Option<ProviderApiKey>, SecretStoreError> {
        unimplemented!("cleanup tests do not read provider api keys")
    }

    fn replace_api_key(
        &self,
        _provider_id: &GpuCloudProviderId,
        _api_key: &ProviderApiKey,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("cleanup tests do not replace provider api keys")
    }

    fn delete_api_key(&self, _provider_id: &GpuCloudProviderId) -> Result<(), SecretStoreError> {
        unimplemented!("cleanup tests do not delete provider api keys")
    }

    fn write_provisioner_worker_token(
        &self,
        _workspace_id: &str,
        _token: &ProvisionerWorkerBearerToken,
    ) -> Result<(), SecretStoreError> {
        unimplemented!("cleanup tests do not write worker tokens")
    }

    fn read_provisioner_worker_token(
        &self,
        _workspace_id: &str,
    ) -> Result<Option<ProvisionerWorkerBearerToken>, SecretStoreError> {
        unimplemented!("cleanup tests do not read worker tokens")
    }

    fn delete_provisioner_worker_token(&self, workspace_id: &str) -> Result<(), SecretStoreError> {
        self.deleted_tokens
            .lock()
            .expect("deleted token lock")
            .push(workspace_id.to_string());
        match &self.delete_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct CleanupProvider {
    calls: Arc<Mutex<Vec<String>>>,
    endpoint_delete_error: Option<ProviderResourceError>,
    template_delete_error: Option<ProviderResourceError>,
    pod_delete_error: Option<ProviderResourceError>,
    volume_delete_error: Option<ProviderResourceError>,
}

impl CleanupProvider {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("provider call lock").clone()
    }

    fn record(&self, call: impl Into<String>) {
        self.calls
            .lock()
            .expect("provider call lock")
            .push(call.into());
    }
}

impl ProviderResourceGateway for CleanupProvider {
    fn create_network_volume<'a>(
        &'a self,
        _input: CreateNetworkVolumeInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not create network volumes") })
    }

    fn get_network_volume<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _volume_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<NetworkVolumeObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not get network volumes") })
    }

    fn discover_network_volumes<'a>(
        &'a self,
        _input: DiscoverNetworkVolumesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<NetworkVolumeObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not discover network volumes") })
    }

    fn delete_network_volume<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        volume_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>> {
        self.record(format!("volume:{volume_id}"));
        let result = self.volume_delete_error.clone();
        Box::pin(async move { result.map_or(Ok(()), Err) })
    }

    fn create_provisioning_pod<'a>(
        &'a self,
        _input: CreateProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not create provisioning pods") })
    }

    fn discover_provisioning_pods<'a>(
        &'a self,
        _input: DiscoverProvisioningPodsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ProvisioningPodObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not discover provisioning pods") })
    }

    fn get_provisioning_pod<'a>(
        &'a self,
        _input: ObserveProvisioningPodInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ProvisioningPodObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not get provisioning pods") })
    }

    fn delete_provisioning_pod<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        pod_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>> {
        self.record(format!("pod:{pod_id}"));
        let result = self.pod_delete_error.clone();
        Box::pin(async move { result.map_or(Ok(()), Err) })
    }

    fn create_endpoint_template<'a>(
        &'a self,
        _input: CreateEndpointTemplateInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not create endpoint templates") })
    }

    fn discover_endpoint_templates<'a>(
        &'a self,
        _input: DiscoverEndpointTemplatesInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<EndpointTemplateObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not discover endpoint templates") })
    }

    fn get_endpoint_template<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _template_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<EndpointTemplateObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not get endpoint templates") })
    }

    fn delete_endpoint_template<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        template_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>> {
        self.record(format!("template:{template_id}"));
        let result = self.template_delete_error.clone();
        Box::pin(async move { result.map_or(Ok(()), Err) })
    }

    fn create_serverless_endpoint<'a>(
        &'a self,
        _input: CreateServerlessEndpointInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not create serverless endpoints") })
    }

    fn discover_serverless_endpoints<'a>(
        &'a self,
        _input: DiscoverServerlessEndpointsInput,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<ServerlessEndpointObservation>, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not discover serverless endpoints") })
    }

    fn get_serverless_endpoint<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        _endpoint_id: &'a str,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ServerlessEndpointObservation, ProviderResourceError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async { panic!("cleanup tests do not get serverless endpoints") })
    }

    fn delete_serverless_endpoint<'a>(
        &'a self,
        _provider_id: GpuCloudProviderId,
        endpoint_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ProviderResourceError>> + Send + 'a>> {
        self.record(format!("endpoint:{endpoint_id}"));
        let result = self.endpoint_delete_error.clone();
        Box::pin(async move { result.map_or(Ok(()), Err) })
    }
}
