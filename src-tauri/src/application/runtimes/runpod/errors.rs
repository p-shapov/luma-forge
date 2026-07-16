use crate::application::runtimes::RuntimeError;

use super::ports::{RunpodRuntimeCatalogError, RunpodRuntimeProviderError};

impl From<RunpodRuntimeProviderError> for RuntimeError {
    fn from(error: RunpodRuntimeProviderError) -> Self {
        match error {
            RunpodRuntimeProviderError::Unauthorized => Self::InvalidCredential,
            RunpodRuntimeProviderError::Unavailable
            | RunpodRuntimeProviderError::CreateOutcomeUnknown
            | RunpodRuntimeProviderError::ObserveUnavailable
            | RunpodRuntimeProviderError::ProvisionerFailed => Self::ProviderUnavailable,
        }
    }
}

impl From<RunpodRuntimeCatalogError> for RuntimeError {
    fn from(_: RunpodRuntimeCatalogError) -> Self {
        Self::CatalogUnavailable
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::RuntimeError;

    use super::RunpodRuntimeProviderError;

    #[test]
    fn provider_errors_preserve_invalid_credentials() {
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::Unauthorized),
            RuntimeError::InvalidCredential
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::Unavailable),
            RuntimeError::ProviderUnavailable
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::ProvisionerFailed),
            RuntimeError::ProviderUnavailable
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::CreateOutcomeUnknown),
            RuntimeError::ProviderUnavailable
        );
        assert_eq!(
            RuntimeError::from(RunpodRuntimeProviderError::ObserveUnavailable),
            RuntimeError::ProviderUnavailable
        );
    }
}
