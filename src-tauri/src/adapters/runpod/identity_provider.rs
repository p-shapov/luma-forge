use secrecy::SecretString;

use crate::{
    application::secrets::{Identity, SecretIdentityProvider, SecretIdentityProviderError},
    infra::clients::{runpod::RunpodClient, NetworkError},
};

pub struct RunpodIdentityAdapter {
    client: RunpodClient,
}

impl RunpodIdentityAdapter {
    pub fn new(client: RunpodClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl SecretIdentityProvider for RunpodIdentityAdapter {
    async fn identity(
        &self,
        credential: &SecretString,
    ) -> Result<Identity, SecretIdentityProviderError> {
        let response = self
            .client
            .myself(credential)
            .await
            .map_err(map_network_error)?;
        let myself = response
            .myself
            .ok_or(SecretIdentityProviderError::Unavailable)?;
        Ok(Identity {
            key_name: None,
            username: None,
            email: myself.email,
        })
    }
}

fn map_network_error(error: NetworkError) -> SecretIdentityProviderError {
    match error {
        NetworkError::Unauthorized => SecretIdentityProviderError::InvalidCredential,
        _ => SecretIdentityProviderError::Unavailable,
    }
}
