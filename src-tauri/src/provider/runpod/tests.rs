use std::time::Duration;

use serde_json::json;

use crate::{
    domain::provider_setup::{ProviderApiKey, ProviderIdentity},
    provider::{
        error::ProviderClientError,
        runpod::{
            contracts::{GraphQlResponse, RunPodIdentityData, RunPodInventoryData},
            mapper::{identity_from_graphql_response, inventory_from_graphql_response},
            provider_error_from_inventory_status, RunPodClient,
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

#[tokio::test]
async fn identity_request_timeout_maps_to_api_unavailable() {
    let client = RunPodClient::new_for_test(
        "http://192.0.2.1/graphql".to_string(),
        Duration::from_millis(50),
    );
    let api_key = ProviderApiKey::new("rp_123_secret".to_string()).expect("valid api key");

    let error = tokio::time::timeout(Duration::from_secs(2), client.validate_identity(&api_key))
        .await
        .expect("request should be bounded")
        .expect_err("transport failure should fail identity validation");

    assert_eq!(error, ProviderClientError::ApiUnavailable);
}

#[tokio::test]
async fn inventory_request_timeout_maps_to_api_unavailable() {
    let client = RunPodClient::new_for_test(
        "http://192.0.2.1/graphql".to_string(),
        Duration::from_millis(50),
    );
    let api_key = ProviderApiKey::new("rp_123_secret".to_string()).expect("valid api key");

    let error = tokio::time::timeout(Duration::from_secs(2), client.fetch_inventory(&api_key))
        .await
        .expect("request should be bounded")
        .expect_err("transport failure should fail inventory fetch");

    assert_eq!(error, ProviderClientError::ApiUnavailable);
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

#[test]
fn maps_inventory_auth_status_to_unauthorized() {
    assert_eq!(
        provider_error_from_inventory_status(reqwest::StatusCode::UNAUTHORIZED),
        Some(ProviderClientError::Unauthorized)
    );
    assert_eq!(
        provider_error_from_inventory_status(reqwest::StatusCode::FORBIDDEN),
        Some(ProviderClientError::Unauthorized)
    );
}

#[test]
fn maps_inventory_auth_graphql_errors_to_unauthorized() {
    let response: GraphQlResponse<RunPodInventoryData> = serde_json::from_value(json!({
        "errors": [
            { "message": "Forbidden" }
        ]
    }))
    .expect("response should parse");

    let error = inventory_from_graphql_response(response).expect_err("auth error should fail");

    assert_eq!(error, ProviderClientError::Unauthorized);
}
