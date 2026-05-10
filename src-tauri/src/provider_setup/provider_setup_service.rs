use std::{future::Future, pin::Pin};

use crate::{
    domain::provider_setup::{
        self, GpuCloudProviderId as DomainGpuCloudProviderId,
        GpuCloudProviderSetup as DomainGpuCloudProviderSetup, ProviderApiKey, ProviderIdentity,
    },
    secrets::SecretStore,
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
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<Option<DomainGpuCloudProviderSetup>, ProviderSetupError> {
        let Some(api_key) = self.secrets.read_api_key(&provider_id)? else {
            return Ok(None);
        };

        let setup = self.setup_from_key(&provider_id, &api_key).await?;

        Ok(Some(setup))
    }

    pub async fn setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
        api_key: ProviderApiKey,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        if self.secrets.read_api_key(&provider_id)?.is_some() {
            return Err(ProviderSetupError::ProviderSetupAlreadyExists);
        }

        self.providers
            .validate_identity(&provider_id, &api_key)
            .await?;
        self.secrets.replace_api_key(&provider_id, &api_key)?;

        let setup = match self.finalize_setup_from_stored_key(&provider_id).await {
            Ok(setup) => setup,
            Err(error) => return Err(self.rollback_failed_setup(&provider_id, error)),
        };

        Ok(setup)
    }

    pub fn delete_setup(
        &self,
        provider_id: DomainGpuCloudProviderId,
    ) -> Result<(), ProviderSetupError> {
        if !self.secrets.has_api_key_entry(&provider_id)? {
            return Err(ProviderSetupError::ProviderSetupIncomplete);
        }

        self.secrets.delete_api_key(&provider_id)?;

        Ok(())
    }

    async fn finalize_setup_from_stored_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        let stored_api_key = self
            .secrets
            .read_api_key(provider_id)?
            .ok_or(ProviderSetupError::SecureKeyringUnavailable)?;

        self.setup_from_key(provider_id, &stored_api_key).await
    }

    fn rollback_failed_setup(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        finalization_error: ProviderSetupError,
    ) -> ProviderSetupError {
        match self.secrets.delete_api_key(provider_id) {
            Ok(()) => finalization_error,
            Err(_) => ProviderSetupError::ProviderSetupRecoveryRequired,
        }
    }

    async fn setup_from_key(
        &self,
        provider_id: &DomainGpuCloudProviderId,
        api_key: &ProviderApiKey,
    ) -> Result<DomainGpuCloudProviderSetup, ProviderSetupError> {
        let identity = self
            .providers
            .validate_identity(provider_id, api_key)
            .await?;
        provider_setup::validator::validate_provider_identity(&identity)
            .map_err(|_| ProviderSetupError::ProviderIdentityUnavailable)?;
        let setup = Self::setup_from_identity(*provider_id, identity);
        provider_setup::validator::validate_gpu_cloud_provider_setup(&setup)
            .map_err(|_| ProviderSetupError::ProviderIdentityUnavailable)?;

        Ok(setup)
    }

    fn setup_from_identity(
        provider_id: DomainGpuCloudProviderId,
        identity: ProviderIdentity,
    ) -> DomainGpuCloudProviderSetup {
        DomainGpuCloudProviderSetup {
            gpu_cloud_provider_id: provider_id,
            provider_user_email: identity.provider_user_email,
            provider_api_key_fingerprint: identity.provider_api_key_fingerprint,
        }
    }
}

#[cfg(test)]
#[path = "provider_setup_tests.rs"]
mod provider_setup_tests;
