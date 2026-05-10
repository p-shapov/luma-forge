use crate::{
    provider_setup,
    shared_contracts::provider_contracts::GpuCloudProviderId as ApplicationGpuCloudProviderId,
};

use super::*;

#[test]
fn maps_setup_request_to_application_contract() {
    let request: provider_setup::SetupGpuCloudProviderRequest = SetupGpuCloudProviderRequest {
        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
        provider_api_key: "rp-key".to_string(),
    }
    .into();

    assert_eq!(
        request.gpu_cloud_provider_id,
        ApplicationGpuCloudProviderId::Runpod
    );
    assert_eq!(request.provider_api_key, "rp-key");
}

#[test]
fn setup_request_debug_redacts_provider_api_key() {
    let debug_output = format!(
        "{:?}",
        SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            provider_api_key: "rp-secret-key".to_string(),
        }
    );

    assert!(debug_output.contains("<redacted>"));
    assert!(!debug_output.contains("rp-secret-key"));
}

#[test]
fn application_setup_request_debug_redacts_provider_api_key() {
    let debug_output = format!(
        "{:?}",
        provider_setup::SetupGpuCloudProviderRequest {
            gpu_cloud_provider_id: ApplicationGpuCloudProviderId::Runpod,
            provider_api_key: "rp-secret-key".to_string(),
        }
    );

    assert!(debug_output.contains("<redacted>"));
    assert!(!debug_output.contains("rp-secret-key"));
}

#[test]
fn maps_setup_response_to_command_contract() {
    let response =
        SetupGpuCloudProviderResponse::from(provider_setup::SetupGpuCloudProviderResponse {
            gpu_cloud_provider_setup: provider_setup::GpuCloudProviderSetup {
                gpu_cloud_provider_id: ApplicationGpuCloudProviderId::Runpod,
                provider_user_email: "user@example.com".to_string(),
                provider_api_key_fingerprint: "rp_123".to_string(),
            },
        });

    assert_eq!(
        response.gpu_cloud_provider_setup.gpu_cloud_provider_id,
        GpuCloudProviderId::Runpod
    );
    assert_eq!(
        response
            .gpu_cloud_provider_setup
            .provider_api_key_fingerprint,
        "rp_123"
    );
}
