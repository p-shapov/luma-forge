mod runpod;

use std::{future::Future, pin::Pin};

use crate::{
    domain::{provider_setup::GpuCloudProviderId, provider_setup::ProviderApiKey},
    provider::ProviderClientError,
};

use super::{contracts::ProviderPlacementOptions, error::WorkspaceSetupError};

pub trait WorkspaceSetupProviderCapability: Send + Sync {
    fn get_provider_placement_options<'a>(
        &'a self,
        api_key: &'a ProviderApiKey,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>> + Send + 'a>,
    >;
}

pub trait WorkspaceSetupProviderResolver: Send + Sync {
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceSetupProviderCapability;
}

#[derive(Debug, Default)]
pub struct WorkspaceSetupProviderRegistry {
    runpod: runpod::RunPodWorkspaceSetupProvider,
}

impl WorkspaceSetupProviderResolver for WorkspaceSetupProviderRegistry {
    fn for_provider(
        &self,
        provider_id: &GpuCloudProviderId,
    ) -> &dyn WorkspaceSetupProviderCapability {
        match provider_id {
            GpuCloudProviderId::Runpod => &self.runpod,
        }
    }
}

pub(in crate::workspace_setup) fn workspace_setup_error_from_client_error(
    error: ProviderClientError,
) -> WorkspaceSetupError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_selects_runpod_capability_for_runpod_provider() {
        let registry = WorkspaceSetupProviderRegistry::default();

        let capability = registry.for_provider(&GpuCloudProviderId::Runpod);

        assert!(std::ptr::addr_eq(
            capability as *const dyn WorkspaceSetupProviderCapability,
            &registry.runpod as &dyn WorkspaceSetupProviderCapability
                as *const dyn WorkspaceSetupProviderCapability,
        ));
    }

    #[test]
    fn maps_provider_client_errors_to_workspace_setup_errors() {
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::Unauthorized),
            WorkspaceSetupError::ProviderApiKeyUnauthorized
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::ApiUnavailable),
            WorkspaceSetupError::ProviderApiUnavailable
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::RateLimited),
            WorkspaceSetupError::ProviderRateLimited
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::RequestRejected),
            WorkspaceSetupError::ProviderRequestRejected
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::ResponseInvalid),
            WorkspaceSetupError::ProviderResponseInvalid
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::NotFound),
            WorkspaceSetupError::ProviderResponseInvalid
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::Conflict),
            WorkspaceSetupError::ProviderResponseInvalid
        );
        assert_eq!(
            workspace_setup_error_from_client_error(ProviderClientError::Indeterminate),
            WorkspaceSetupError::ProviderResponseInvalid
        );
    }
}
