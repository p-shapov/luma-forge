use crate::domain::provider_setup as domain_provider_setup;

use super::*;

#[test]
fn maps_setup_request_provider_id_to_domain() {
    let request = SetupGpuCloudProviderRequest {
        gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId::Runpod,
        provider_api_key: "rp-key".to_string(),
    };

    assert_eq!(
        request.gpu_cloud_provider_id,
        domain_provider_setup::GpuCloudProviderId::Runpod
    );
}

#[test]
fn setup_request_debug_redacts_provider_api_key() {
    let debug_output = format!(
        "{:?}",
        SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId::Runpod,
            provider_api_key: "rp-secret-key".to_string(),
        }
    );

    assert!(debug_output.contains("<redacted>"));
    assert!(!debug_output.contains("rp-secret-key"));
}

#[test]
fn maps_setup_response_to_command_contract() {
    let response = SetupGpuCloudProviderResponse {
        gpu_cloud_provider_setup: domain_provider_setup::GpuCloudProviderSetup {
            gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId::Runpod,
            provider_user_email: "user@example.com".to_string(),
            provider_api_key_fingerprint: "rp_123".to_string(),
        },
    };

    assert_eq!(
        response.gpu_cloud_provider_setup.gpu_cloud_provider_id,
        domain_provider_setup::GpuCloudProviderId::Runpod
    );
    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .provider_api_key_fingerprint,
        "rp_123"
    );
}
