use crate::domain::provider::GpuCloudProviderId;

use super::{errors::RemoteWorkspaceProviderRegistryError, provider::RemoteWorkspaceProvider};

pub struct RemoteWorkspaceProviderRegistry {
    providers: Vec<Box<dyn RemoteWorkspaceProvider>>,
}

impl RemoteWorkspaceProviderRegistry {
    pub fn new(providers: Vec<Box<dyn RemoteWorkspaceProvider>>) -> Self {
        Self { providers }
    }

    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn for_provider(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<&dyn RemoteWorkspaceProvider, RemoteWorkspaceProviderRegistryError> {
        self.providers
            .iter()
            .find(|provider| provider.provider_id() == provider_id)
            .map(|provider| provider.as_ref())
            .ok_or(RemoteWorkspaceProviderRegistryError::MissingProvider { provider_id })
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{
        provider::GpuCloudProviderId,
        workspace::{
            RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
            RemoteVolumeSnapshot,
        },
    };

    use super::*;
    use crate::remote_workspace::{
        errors::{
            CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
            GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError,
            ObserveVolumeError, StartProvisionerError, TerminateProvisionerError,
        },
        provider::{
            CreateEndpointParams, CreateVolumeParams, DeleteEndpointParams, DeleteVolumeParams,
            GetProvisionerStatusParams, ObserveEndpointParams, ObserveProvisionerParams,
            ObserveVolumeParams, ProviderFuture, RemoteEndpointProvider, RemoteProvisionerProvider,
            RemoteVolumeProvider, StartProvisionerParams, TerminateProvisionerParams,
        },
    };

    struct FakeProvider {
        provider_id: GpuCloudProviderId,
    }

    impl RemoteVolumeProvider for FakeProvider {
        fn create_volume<'a>(
            &'a self,
            _params: CreateVolumeParams,
        ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>> {
            Box::pin(async {
                Ok(RemoteVolumeSnapshot {
                    id: "volume".to_string(),
                })
            })
        }

        fn delete_volume<'a>(
            &'a self,
            _params: DeleteVolumeParams,
        ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_volume<'a>(
            &'a self,
            _params: ObserveVolumeParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>> {
            Box::pin(async { Ok(None) })
        }
    }

    impl RemoteProvisionerProvider for FakeProvider {
        fn start_provisioner<'a>(
            &'a self,
            _params: StartProvisionerParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>> {
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
        ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_provisioner<'a>(
            &'a self,
            _params: ObserveProvisionerParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>
        {
            Box::pin(async { Ok(None) })
        }

        fn get_provisioner_status<'a>(
            &'a self,
            _params: GetProvisionerStatusParams,
        ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>>
        {
            Box::pin(async { Ok(RemoteProvisionerStatus::Pending) })
        }
    }

    impl RemoteEndpointProvider for FakeProvider {
        fn create_endpoint<'a>(
            &'a self,
            _params: CreateEndpointParams,
        ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>> {
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
        ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>> {
            Box::pin(async { Ok(()) })
        }

        fn observe_endpoint<'a>(
            &'a self,
            _params: ObserveEndpointParams,
        ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>>
        {
            Box::pin(async { Ok(None) })
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
            RemoteWorkspaceProviderRegistryError::MissingProvider {
                provider_id: GpuCloudProviderId::Runpod
            }
        );
    }
}
