use thiserror::Error;

use crate::secrets::SecretStoreError;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceProvisioningError {
    #[error("workspace not found")]
    WorkspaceNotFound,
    #[error("invalid workspace lifecycle")]
    InvalidWorkspaceLifecycle,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api key unauthorized")]
    ProviderApiKeyUnauthorized,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("provider rate limited")]
    ProviderRateLimited,
    #[error("provider request rejected")]
    ProviderRequestRejected,
    #[error("provider response invalid")]
    ProviderResponseInvalid,
    #[error("provider resource not found")]
    ProviderResourceNotFound,
    #[error("provider operation conflict")]
    ProviderOperationConflict,
    #[error("provider operation indeterminate")]
    ProviderOperationIndeterminate,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("provisioner worker token invalid")]
    ProvisionerWorkerTokenInvalid,
    #[error("provisioner worker unauthorized")]
    ProvisionerWorkerUnauthorized,
    #[error("provisioner worker unavailable")]
    ProvisionerWorkerUnavailable,
    #[error("provisioner worker conflict")]
    ProvisionerWorkerConflict,
    #[error("provisioner worker response invalid")]
    ProvisionerWorkerResponseInvalid,
    #[error("provisioner worker failed")]
    ProvisionerWorkerFailed { diagnostic: Option<String> },
}

impl From<SecretStoreError> for WorkspaceProvisioningError {
    fn from(error: SecretStoreError) -> Self {
        match error {
            SecretStoreError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            SecretStoreError::InvalidStoredProviderApiKey => Self::ProviderSetupIncomplete,
            SecretStoreError::InvalidStoredProvisionerWorkerToken => {
                Self::ProvisionerWorkerTokenInvalid
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_provisioning_token_secret_failures_to_provisioning_errors() {
        assert_eq!(
            WorkspaceProvisioningError::from(SecretStoreError::InvalidStoredProvisionerWorkerToken),
            WorkspaceProvisioningError::ProvisionerWorkerTokenInvalid
        );
        assert_eq!(
            WorkspaceProvisioningError::from(SecretStoreError::SecureKeyringUnavailable),
            WorkspaceProvisioningError::SecureKeyringUnavailable
        );
    }
}
