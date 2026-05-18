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
}
