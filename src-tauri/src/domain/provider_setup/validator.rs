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
    use super::*;
    use crate::domain::provider_setup::GpuCloudProviderId;

    fn valid_identity() -> ProviderIdentity {
        ProviderIdentity {
            provider_user_email: "user@example.com".to_string(),
            provider_api_key_fingerprint: "runpod-key-fingerprint".to_string(),
        }
    }

    #[test]
    fn validate_provider_identity_accepts_non_blank_printable_fields() {
        assert_eq!(validate_provider_identity(&valid_identity()), Ok(()));
    }

    #[test]
    fn validate_provider_identity_rejects_blank_or_control_fields() {
        let invalid_identities = [
            ProviderIdentity {
                provider_user_email: " ".to_string(),
                ..valid_identity()
            },
            ProviderIdentity {
                provider_api_key_fingerprint: "\t".to_string(),
                ..valid_identity()
            },
            ProviderIdentity {
                provider_user_email: "user\n@example.com".to_string(),
                ..valid_identity()
            },
            ProviderIdentity {
                provider_api_key_fingerprint: "fingerprint\r".to_string(),
                ..valid_identity()
            },
        ];

        for identity in invalid_identities {
            assert_eq!(
                validate_provider_identity(&identity),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_gpu_cloud_provider_setup_uses_same_identity_rules() {
        let setup = GpuCloudProviderSetup {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_user_email: "user@example.com".to_string(),
            provider_api_key_fingerprint: "runpod-key-fingerprint".to_string(),
        };
        assert_eq!(validate_gpu_cloud_provider_setup(&setup), Ok(()));

        let invalid_setup = GpuCloudProviderSetup {
            provider_api_key_fingerprint: "\n".to_string(),
            ..setup
        };
        assert_eq!(
            validate_gpu_cloud_provider_setup(&invalid_setup),
            Err(DomainValidationError)
        );
    }
}
