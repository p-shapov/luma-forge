use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    validation::is_blank,
};

use super::{GpuCloudProviderSetup, ProviderIdentity};

pub fn validate_provider_identity(identity: &ProviderIdentity) -> DomainValidationResult {
    if is_blank(&identity.provider_user_email)
        || is_blank(&identity.provider_api_key_fingerprint)
        || contains_control_character(&identity.provider_user_email)
        || contains_control_character(&identity.provider_api_key_fingerprint)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

pub fn validate_gpu_cloud_provider_setup(setup: &GpuCloudProviderSetup) -> DomainValidationResult {
    if is_blank(&setup.provider_user_email)
        || is_blank(&setup.provider_api_key_fingerprint)
        || contains_control_character(&setup.provider_user_email)
        || contains_control_character(&setup.provider_api_key_fingerprint)
    {
        return Err(DomainValidationError);
    }

    Ok(())
}

fn contains_control_character(value: &str) -> bool {
    value.chars().any(char::is_control)
}
