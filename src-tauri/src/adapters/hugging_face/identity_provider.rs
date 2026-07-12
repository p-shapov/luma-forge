use secrecy::SecretString;

use crate::{
    application::secrets::{Identity, SecretIdentityProvider, SecretIdentityProviderError},
    infra::clients::{hugging_face::HuggingFaceClient, NetworkError},
};

pub struct HuggingFaceIdentityAdapter {
    client: HuggingFaceClient,
}

impl HuggingFaceIdentityAdapter {
    pub fn new(client: HuggingFaceClient) -> Self {
        Self { client }
    }
}

#[crate::diagnostics::diagnostic]
#[async_trait::async_trait]
impl SecretIdentityProvider for HuggingFaceIdentityAdapter {
    #[diagnostic(show_output, show_error)]
    async fn identity(
        &self,
        #[diagnostic(redact)] credential: &SecretString,
    ) -> Result<Identity, SecretIdentityProviderError> {
        let response = self
            .client
            .whoami(credential)
            .await
            .map_err(map_network_error)?;
        Ok(Identity {
            key_name: response.auth.access_token.map(|token| token.display_name),
            username: Some(response.name),
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
