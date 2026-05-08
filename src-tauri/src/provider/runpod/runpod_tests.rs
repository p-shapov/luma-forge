use serde_json::json;

use crate::{
    domain::provider_setup::{ProviderApiKey, ProviderIdentity},
    provider::{
        provider_client_error::ProviderClientError,
        runpod::{
            runpod_contracts::{GraphQlResponse, RunPodIdentityData, RunPodInventoryData},
            runpod_mapper::{identity_from_graphql_response, inventory_from_graphql_response},
        },
    },
};

fn parse_identity(
    secret: &str,
    value: serde_json::Value,
) -> Result<ProviderIdentity, ProviderClientError> {
    let response: GraphQlResponse<RunPodIdentityData> =
        serde_json::from_value(value).expect("response should parse");
    identity_from_graphql_response(&ProviderApiKey::new(secret.to_string()).unwrap(), response)
}

#[test]
fn parses_identity_with_single_active_prefix_match() {
    let identity = parse_identity(
        "rp_123_secret",
        json!({
            "data": {
                "myself": {
                    "email": "user@example.com",
                    "apiKeys": [
                        { "id": "rp_123", "isActive": true },
                        { "id": "rp_999", "isActive": true }
                    ]
                }
            }
        }),
    )
    .expect("identity should parse");

    assert_eq!(identity.provider_user_email, "user@example.com");
    assert_eq!(identity.provider_api_key_fingerprint, "rp_123");
}

#[test]
fn rejects_inactive_matched_key() {
    let error = parse_identity(
        "rp_123_secret",
        json!({
            "data": {
                "myself": {
                    "email": "user@example.com",
                    "apiKeys": [
                        { "id": "rp_123", "isActive": false }
                    ]
                }
            }
        }),
    )
    .expect_err("inactive key should fail");

    assert_eq!(error, ProviderClientError::Unauthorized);
}

#[test]
fn rejects_missing_prefix_match() {
    let error = parse_identity(
        "rp_123_secret",
        json!({
            "data": {
                "myself": {
                    "email": "user@example.com",
                    "apiKeys": [
                        { "id": "rp_999", "isActive": true }
                    ]
                }
            }
        }),
    )
    .expect_err("missing match should fail");

    assert_eq!(error, ProviderClientError::IdentityUnavailable);
}

#[test]
fn rejects_ambiguous_prefix_match() {
    let error = parse_identity(
        "rp_123_secret",
        json!({
            "data": {
                "myself": {
                    "email": "user@example.com",
                    "apiKeys": [
                        { "id": "rp_", "isActive": true },
                        { "id": "rp_123", "isActive": true }
                    ]
                }
            }
        }),
    )
    .expect_err("ambiguous match should fail");

    assert_eq!(error, ProviderClientError::IdentityUnavailable);
}

#[test]
fn maps_auth_graphql_errors_to_invalid_key() {
    let error = parse_identity(
        "rp_123_secret",
        json!({
            "errors": [
                { "message": "Unauthorized" }
            ]
        }),
    )
    .expect_err("auth errors should fail");

    assert_eq!(error, ProviderClientError::Unauthorized);
}

#[test]
fn parses_inventory_response() {
    let response: GraphQlResponse<RunPodInventoryData> = serde_json::from_value(json!({
        "data": {
            "dataCenters": [
                {
                    "id": "EU-RO-1",
                    "name": "EU RO 1",
                    "gpuAvailability": [
                        {
                            "stockStatus": "High",
                            "gpuType": {
                                "id": "NVIDIA RTX 4090",
                                "displayName": "RTX 4090",
                                "memoryInGb": 24
                            }
                        }
                    ]
                }
            ]
        }
    }))
    .expect("inventory should parse");

    let inventory = inventory_from_graphql_response(response).expect("inventory should map");

    assert_eq!(inventory.datacenters.len(), 1);
    assert_eq!(
        inventory.datacenters[0].gpu_options[0].vram_bytes,
        24 * 1024 * 1024 * 1024
    );
    assert_eq!(
        inventory.datacenters[0].gpu_options[0].availability_score,
        100
    );
}
