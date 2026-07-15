use secrecy::SecretString;

use crate::{
    application::secrets::{Identity, SecretIdentityProvider, SecretIdentityProviderError},
    providers::{
        runpod::{IdentityRequest, RunpodProvider},
        NetworkError,
    },
};

pub struct RunpodIdentityAdapter {
    provider: RunpodProvider,
}

impl RunpodIdentityAdapter {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            provider: RunpodProvider::new()?,
        })
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl SecretIdentityProvider for RunpodIdentityAdapter {
    #[diagnostic(show_output, show_error)]
    async fn identity(
        &self,
        #[diagnostic(redact)] credential: &SecretString,
    ) -> Result<Identity, SecretIdentityProviderError> {
        let response = self
            .provider
            .identity(IdentityRequest {
                credential: credential.clone(),
            })
            .await
            .map_err(map_network_error)?;
        Ok(Identity {
            key_name: None,
            username: None,
            email: response.email,
        })
    }
}

fn map_network_error(error: NetworkError) -> SecretIdentityProviderError {
    match error {
        NetworkError::Unauthorized => SecretIdentityProviderError::InvalidCredential,
        _ => SecretIdentityProviderError::Unavailable,
    }
}
