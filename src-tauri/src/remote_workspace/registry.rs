use crate::domain::provider::GpuCloudProviderId;

use super::{errors::RemoteWorkspaceError, provider::RemoteWorkspaceProvider};

pub struct RemoteWorkspaceProviderRegistry {
    providers: Vec<Box<dyn RemoteWorkspaceProvider>>,
}

impl RemoteWorkspaceProviderRegistry {
    pub fn new(providers: Vec<Box<dyn RemoteWorkspaceProvider>>) -> Self {
        Self { providers }
    }

    pub fn with_provider(provider: Box<dyn RemoteWorkspaceProvider>) -> Self {
        Self {
            providers: vec![provider],
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
    ) -> Result<&dyn RemoteWorkspaceProvider, RemoteWorkspaceError> {
        self.providers
            .iter()
            .find(|provider| provider.provider_id() == provider_id)
            .map(|provider| provider.as_ref())
            .ok_or(RemoteWorkspaceError::ProviderUnavailable { provider_id })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        placement::{
            RemoteDatacenterPlacementOption, RemoteGpuPlacementOption, RemotePlacementOptions,
        },
        provider::GpuCloudProviderId,
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteVolumeSnapshot,
        },
    };

    use super::*;
    use crate::remote_workspace::{
        errors::RemoteWorkspaceError,
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, RemoteEndpointProvider, RemotePlacementOptionsProvider,
            RemoteProvisionerProvider, RemoteVolumeProvider, StartProvisionerParams,
            TerminateProvisionerParams,
        },
        providers::runpod::RunpodRemoteWorkspaceProvider,
    };
    use crate::secrets_storage::{
        ApiKeyIdentityProvider, ApiSecret, SecretKey, SecretStore, SecretsStorageError,
        SecretsStorageService,
    };
    use crate::shared::AppFuture;
    use std::{collections::HashMap, sync::Arc};

    struct FakeProvider {
        provider_id: GpuCloudProviderId,
    }

    impl RemotePlacementOptionsProvider for FakeProvider {
        fn get_provider_placement_options<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>> {
            Box::pin(async {
                Ok(RemotePlacementOptions {
                    max_persistent_storage_volume_size_bytes: Some(10),
                    datacenters: vec![RemoteDatacenterPlacementOption {
                        id: "dc".to_string(),
                        name: "Datacenter".to_string(),
                        gpu_options: vec![RemoteGpuPlacementOption {
                            id: "gpu".to_string(),
                            name: "GPU".to_string(),
                            vram_bytes: 24,
                            availability_score: 90,
                        }],
                    }],
                })
            })
        }
    }

    impl RemoteVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            _params: CreateVolumeParams,
        ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>> {
            Box::pin(async {
                Ok(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            _params: StartProvisionerParams,
        ) -> AppFuture<'a, Result<RemoteProvisionerSnapshot, RemoteWorkspaceError>> {
            Box::pin(async {
                Ok(RemoteProvisionerSnapshot {
                    id: "provisioner".to_string(),
                    status_url: "https://status.example".to_string(),
                })
            })
        }

        fn terminate_provisioner<'a>(
            &'a self,
            _params: TerminateProvisionerParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async { Ok(()) })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            _params: GetProvisionerStatusParams,
        ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>> {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>> {
            Box::pin(async {
                Ok(RemoteEndpointSnapshot {
                    id: "endpoint".to_string(),
                    url: "https://endpoint.example".to_string(),
                })
            })
        }

        fn delete_endpoint<'a>(
            &'a self,
            _params: DeleteEndpointParams,
        ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>> {
            Box::pin(async { Ok(()) })
        }
    }

    impl RemoteWorkspaceProvider for FakeProvider {
        fn provider_id(&self) -> GpuCloudProviderId {
            self.provider_id
        }
    }

    #[test]
    fn lookup_returns_registered_provider() {
        let registry = RemoteWorkspaceProviderRegistry::new(vec![Box::new(FakeProvider {
            provider_id: GpuCloudProviderId::Runpod,
        })]);

        let provider = registry
            .for_provider(GpuCloudProviderId::Runpod)
            .expect("registered provider should resolve");

        assert_eq!(provider.provider_id(), GpuCloudProviderId::Runpod);
    }

    #[test]
    fn with_provider_resolves_runpod_provider() {
        let store = FakeSecretStore::default();
        let registry = RemoteWorkspaceProviderRegistry::with_provider(Box::new(
            RunpodRemoteWorkspaceProvider::new(
                SecretsStorageService::new(
                    store.clone(),
                    FakeIdentityProvider,
                    SecretKey::RunpodApiKey,
                ),
                SecretsStorageService::new(
                    store,
                    FakeIdentityProvider,
                    SecretKey::HuggingFaceApiKey,
                ),
            ),
        ));

        let provider = registry
            .for_provider(GpuCloudProviderId::Runpod)
            .expect("runpod provider should resolve");

        assert_eq!(provider.provider_id(), GpuCloudProviderId::Runpod);
    }

    #[test]
    fn missing_provider_returns_explicit_error() {
        let registry = RemoteWorkspaceProviderRegistry::empty();

        let error = match registry.for_provider(GpuCloudProviderId::Runpod) {
            Ok(provider) => panic!(
                "missing provider should fail, resolved {:?}",
                provider.provider_id()
            ),
            Err(error) => error,
        };

        assert_eq!(
            error,
            RemoteWorkspaceError::ProviderUnavailable {
                provider_id: GpuCloudProviderId::Runpod
            }
        );
    }

    #[derive(Clone, Default)]
    struct FakeSecretStore {
        secrets: Arc<HashMap<SecretKey, ApiSecret>>,
    }

    impl SecretStore for FakeSecretStore {
        fn has<'a>(&'a self, key: SecretKey) -> AppFuture<'a, Result<bool, SecretsStorageError>> {
            Box::pin(async move { Ok(self.secrets.contains_key(&key)) })
        }

        fn write<'a>(
            &'a self,
            _key: SecretKey,
            _secret: ApiSecret,
        ) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(&'a self, _key: SecretKey) -> AppFuture<'a, Result<(), SecretsStorageError>> {
            Box::pin(async { Ok(()) })
        }

        fn read<'a>(
            &'a self,
            key: SecretKey,
        ) -> AppFuture<'a, Result<Option<ApiSecret>, SecretsStorageError>> {
            Box::pin(async move { Ok(self.secrets.get(&key).cloned()) })
        }
    }

    #[derive(Clone)]
    struct FakeIdentityProvider;

    impl ApiKeyIdentityProvider for FakeIdentityProvider {
        fn identity<'a>(
            &'a self,
            _secret: &'a ApiSecret,
        ) -> AppFuture<'a, Result<crate::domain::secrets::ApiKeyIdentity, SecretsStorageError>>
        {
            Box::pin(async {
                Ok(crate::domain::secrets::ApiKeyIdentity {
                    email: None,
                    username: None,
                    key_display_name: None,
                })
            })
        }
    }
}
