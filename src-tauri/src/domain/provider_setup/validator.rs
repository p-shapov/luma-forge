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

#[cfg(test)]
mod tests {
    use crate::domain::provider_setup::{GpuCloudProviderId, GpuCloudProviderSetup};

    use super::*;

    #[test]
    fn rejects_blank_provider_identity_fields() {
        let identity = ProviderIdentity {
            provider_user_email: " ".to_string(),
            provider_api_key_fingerprint: "rp_123".to_string(),
        };

        let error = validate_provider_identity(&identity)
            .expect_err("blank provider identity email should fail");

        assert_eq!(error, DomainValidationError);
    }

    #[test]
    fn rejects_control_characters_in_setup_snapshot() {
        let setup = GpuCloudProviderSetup {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_user_email: "user@example.com".to_string(),
            provider_api_key_fingerprint: "rp_123\0".to_string(),
        };

        let error = validate_gpu_cloud_provider_setup(&setup)
            .expect_err("control character in setup snapshot should fail");

        assert_eq!(error, DomainValidationError);
    }
}
