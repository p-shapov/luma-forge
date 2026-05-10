use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{
        GpuCloudProviderId as DomainGpuCloudProviderId,
        GpuCloudProviderSetup as DomainGpuCloudProviderSetup, ProviderApiKey, ProviderIdentity,
    },
    secrets::SecretStore,
};

use super::provider_setup_contracts::{
    DeleteGpuCloudProviderSetupRequest, DeleteGpuCloudProviderSetupResponse,
    GetGpuCloudProviderSetupRequest, GetGpuCloudProviderSetupResponse, GpuCloudProviderSetup,
    SetupGpuCloudProviderRequest, SetupGpuCloudProviderResponse,
};
use super::provider_setup_error::ProviderSetupError;

pub trait ProviderIdentityGateway: Send + Sync {
    fn validate_identity<'a>(
        &'a self,
        provider_id: &'a DomainGpuCloudProviderId,
        api_key: &'a ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderIdentity, ProviderSetupError>> + Send + 'a>>;
}

pub struct ProviderSetupService<S, P> {
    secrets: S,
    providers: P,
}

impl<S, P> ProviderSetupService<S, P> {
    pub fn new(secrets: S, providers: P) -> Self {
        Self { secrets, providers }
    }
}

impl<S, P> ProviderSetupService<S, P>
where
    S: SecretStore,
    P: ProviderIdentityGateway,
{
    pub async fn get_setup(
        &self,
        request: GetGpuCloudProviderSetupRequest,
    ) -> Result<GetGpuCloudProviderSetupResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id.into();
        let Some(api_key) = self.secrets.read_api_key(&provider_id)? else {
            return Ok(GetGpuCloudProviderSetupResponse {
                gpu_cloud_provider_setup: None,
            });
        };

        let setup = self.setup_from_key(&provider_id, &api_key).await?;

        Ok(GetGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: Some(setup),
        })
    }

    pub async fn setup(
        &self,
        request: SetupGpuCloudProviderRequest,
    ) -> Result<SetupGpuCloudProviderResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id.into();
        if self.secrets.read_api_key(&provider_id)?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        let api_key = ProviderApiKey::new(request.provider_api_key)
            .map_err(|_| ProviderSetupError::InvalidProviderApiKey)?;

        self.providers
            .validate_identity(&provider_id, &api_key)
            .await?;
        self.secrets.replace_api_key(&provider_id, &api_key)?;

        let stored_api_key = self
            .secrets
            .read_api_key(&provider_id)?
            .ok_or(ProviderSetupError::SecureKeyringUnavailable)?;
        let setup = self.setup_from_key(&provider_id, &stored_api_key).await?;

        Ok(SetupGpuCloudProviderResponse {
            gpu_cloud_provider_setup: setup,
        })
    }

    pub fn delete_setup(
        &self,
        request: DeleteGpuCloudProviderSetupRequest,
    ) -> Result<DeleteGpuCloudProviderSetupResponse, ProviderSetupError> {
        let provider_id = request.gpu_cloud_provider_id.into();
        if self.secrets.read_api_key(&provider_id)?.is_none() {
            return Err(ProviderSetupError::ProviderSetupIncomplete);
        }

        self.secrets.delete_api_key(&provider_id)?;

        Ok(DeleteGpuCloudProviderSetupResponse {
            gpu_cloud_provider_setup: None,
        })
    }

    async fn setup_from_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<GpuCloudProviderSetup, ProviderSetupError> {
        let identity = self
            .providers
            .validate_identity(provider_id, api_key)
            .await?;

        Ok(Self::setup_from_identity(*provider_id, identity))
    }

    fn setup_from_identity(
        provider_id: DomainGpuCloudProviderId,
        identity: ProviderIdentity,
    ) -> GpuCloudProviderSetup {
        DomainGpuCloudProviderSetup {
            gpu_cloud_provider_id: provider_id,
            provider_user_email: identity.provider_user_email,
            provider_api_key_fingerprint: identity.provider_api_key_fingerprint,
        }
        .into()
    }
}

#[cfg(test)]
#[path = "provider_setup_tests.rs"]
mod provider_setup_tests;
