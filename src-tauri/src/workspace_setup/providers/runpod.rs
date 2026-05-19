use std::{future::Future, pin::Pin};

use crate::{
    domain::{
        placement::ProviderPlacementCapabilities,
        provider_setup::{GpuCloudProviderId, ProviderApiKey},
    },
    provider::runpod::RunPodClient,
};

use super::{
    workspace_setup_error_from_client_error, ProviderPlacementOptions, WorkspaceSetupError,
    WorkspaceSetupProviderCapability,
};

#[derive(Debug, Clone, Default)]
pub(super) struct RunPodWorkspaceSetupProvider {
    client: RunPodClient,
}

impl WorkspaceSetupProviderCapability for RunPodWorkspaceSetupProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let provider_inventory = self
                .client
                .fetch_inventory(api_key)
                .await
                .map_err(workspace_setup_error_from_client_error)?;
            let placement_capabilities =
                ProviderPlacementCapabilities::for_provider(GpuCloudProviderId::Runpod);

            Ok(ProviderPlacementOptions {
                provider_inventory,
                placement_capabilities,
            })
        })
    }
}
