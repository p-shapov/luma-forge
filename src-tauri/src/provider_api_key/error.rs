#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderApiKeyError {
    ProviderSetupIncomplete,
    ProviderSetupAlreadyExists,
    StoredProviderApiKeyInvalid,
    SecureKeyringUnavailable,
    ProviderUnauthorized,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderRequestFailed { message: String },
    ProviderIdentityResponseInvalid,
}
