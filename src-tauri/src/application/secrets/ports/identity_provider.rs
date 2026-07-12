use secrecy::SecretString;

use crate::application::secrets::Identity;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretIdentityProviderError {
    #[error("credential is invalid")]
    InvalidCredential,
    #[error("identity provider is unavailable")]
    Unavailable,
}

#[async_trait::async_trait]
pub trait SecretIdentityProvider: Send + Sync {
    async fn identity(
        &self,
        credential: &SecretString,
    ) -> Result<Identity, SecretIdentityProviderError>;
}
