use std::{collections::HashMap, time::Duration};

use serde_json::json;

use crate::{
    domain::{
        provider_setup::{ProviderApiKey, ProviderIdentity},
        workspace::ProviderResourceStatus,
    },
    provider::{
        error::ProviderClientError,
        runpod::{
            contracts::{
                GraphQlResponse, RunPodCreateNetworkVolumeRequest, RunPodCreatePodRequest,
                RunPodCreateTemplateRequest, RunPodEndpointResponse, RunPodIdentityData,
                RunPodInventoryData, RunPodNetworkVolumeResponse, RunPodPodResponse,
                RunPodTemplateResponse,
            },
            mapper::{
                endpoint_from_response, identity_from_graphql_response,
                inventory_from_graphql_response, network_volume_from_response, pod_from_response,
                template_from_response,
            },
            provider_error_from_inventory_status, provider_error_from_rest_status, RunPodClient,
            RUNPOD_REST_ENDPOINT,
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
fn default_rest_endpoint_uses_documented_host() {
    assert_eq!(RUNPOD_REST_ENDPOINT, "https://rest.runpod.io/v1");
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

    assert_eq!(error, ProviderClientError::ResponseInvalid);
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

    assert_eq!(error, ProviderClientError::ResponseInvalid);
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
        "http://127.0.0.1:9/graphql".to_string(),
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
                    "storageSupport": true,
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
fn filters_inventory_datacenters_without_storage_support() {
    let response: GraphQlResponse<RunPodInventoryData> = serde_json::from_value(json!({
        "data": {
            "dataCenters": [
                {
                    "id": "AP-IN-1",
                    "name": "AP IN 1",
                    "storageSupport": false,
                    "gpuAvailability": [
                        {
                            "stockStatus": "High",
                            "gpuType": {
                                "id": "NVIDIA H100 80GB HBM3",
                                "displayName": "H100 SXM",
                                "memoryInGb": 80
                            }
                        }
                    ]
                }
            ]
        }
    }))
    .expect("inventory should parse");

    let inventory = inventory_from_graphql_response(response).expect("inventory should map");

    assert!(inventory.datacenters.is_empty());
}

#[test]
fn filters_inventory_datacenters_when_storage_support_is_missing_or_null() {
    let response: GraphQlResponse<RunPodInventoryData> = serde_json::from_value(json!({
        "data": {
            "dataCenters": [
                {
                    "id": "AP-IN-1",
                    "name": "AP IN 1",
                    "gpuAvailability": [
                        {
                            "stockStatus": "High",
                            "gpuType": {
                                "id": "NVIDIA H100 80GB HBM3",
                                "displayName": "H100 SXM",
                                "memoryInGb": 80
                            }
                        }
                    ]
                },
                {
                    "id": "EU-FR-1",
                    "name": "EU FR 1",
                    "storageSupport": null,
                    "gpuAvailability": [
                        {
                            "stockStatus": "High",
                            "gpuType": {
                                "id": "NVIDIA H200",
                                "displayName": "H200 SXM",
                                "memoryInGb": 141
                            }
                        }
                    ]
                }
            ]
        }
    }))
    .expect("inventory should parse");

    let inventory = inventory_from_graphql_response(response).expect("inventory should map");

    assert!(inventory.datacenters.is_empty());
}

#[test]
fn keeps_storage_supported_inventory_datacenters() {
    let response: GraphQlResponse<RunPodInventoryData> = serde_json::from_value(json!({
        "data": {
            "dataCenters": [
                {
                    "id": "EU-RO-1",
                    "name": "EU RO 1",
                    "storageSupport": true,
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
    assert_eq!(inventory.datacenters[0].id, "EU-RO-1");
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
fn maps_inventory_client_error_status_to_request_rejected() {
    assert_eq!(
        provider_error_from_inventory_status(reqwest::StatusCode::BAD_REQUEST),
        Some(ProviderClientError::RequestRejected)
    );
    assert_eq!(
        provider_error_from_inventory_status(reqwest::StatusCode::UNPROCESSABLE_ENTITY),
        Some(ProviderClientError::RequestRejected)
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

#[test]
fn serializes_network_volume_create_request_with_documented_size_field() {
    let payload = serde_json::to_value(RunPodCreateNetworkVolumeRequest {
        name: "lf-workspace-volume".to_string(),
        data_center_id: "EU-RO-1".to_string(),
        size: 80,
    })
    .expect("request should serialize");

    assert_eq!(
        payload,
        json!({
            "name": "lf-workspace-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80
        })
    );
}

#[test]
fn parses_network_volume_response() {
    let response: RunPodNetworkVolumeResponse = serde_json::from_value(json!({
        "id": "volume-1",
        "name": "lf-workspace",
        "dataCenterId": "EU-RO-1",
        "size": 80
    }))
    .expect("volume response should parse");

    let observation = network_volume_from_response(response).expect("network volume should map");

    assert_eq!(observation.id, "volume-1");
    assert_eq!(observation.data_center_id, "EU-RO-1");
    assert_eq!(observation.size_gb, 80);
    assert_eq!(observation.status, ProviderResourceStatus::Ready);
}

#[test]
fn serializes_pod_create_request_with_documented_shape() {
    let payload = serde_json::to_value(RunPodCreatePodRequest {
        name: "lf-workspace-provisioner".to_string(),
        image_name: "ghcr.io/luma-forge/provisioner-worker:test".to_string(),
        gpu_type_ids: vec!["NVIDIA GeForce RTX 4090".to_string()],
        data_center_ids: vec!["EU-RO-1".to_string()],
        network_volume_id: "volume-1".to_string(),
        volume_mount_path: "/workspace".to_string(),
        env: HashMap::from([(
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN".to_string(),
            "worker-token".to_string(),
        )]),
        ports: vec!["8080/http".to_string()],
    })
    .expect("request should serialize");

    assert_eq!(
        payload,
        json!({
            "name": "lf-workspace-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner-worker:test",
            "gpuTypeIds": ["NVIDIA GeForce RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "networkVolumeId": "volume-1",
            "volumeMountPath": "/workspace",
            "env": {
                "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": "worker-token"
            },
            "ports": ["8080/http"]
        })
    );
}

#[test]
fn parses_pod_response_and_derives_status_url() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "desiredStatus": "RUNNING",
        "machine": {
            "dataCenterId": "EU-RO-1",
            "gpuTypeId": "NVIDIA GeForce RTX 4090"
        },
        "publicIp": "203.0.113.10",
        "portMappings": {
            "8080": 30001
        },
        "ports": [
            "8080/http"
        ]
    }))
    .expect("pod response should parse");

    let observation = pod_from_response(response).expect("pod should map");

    assert_eq!(observation.data_center_id, "EU-RO-1");
    assert_eq!(observation.selected_gpu_id, "NVIDIA GeForce RTX 4090");
    assert_eq!(observation.status, ProviderResourceStatus::Running);
    assert_eq!(
        observation.provisioner_status_url.as_deref(),
        Some("http://203.0.113.10:30001/status")
    );
}

#[test]
fn serializes_serverless_template_create_request_with_worker_port() {
    let payload = serde_json::to_value(RunPodCreateTemplateRequest {
        name: "lf-workspace-endpoint-template".to_string(),
        image_name: "ghcr.io/luma-forge/endpoint-worker:dev".to_string(),
        container_disk_in_gb: 10,
        env: HashMap::new(),
        is_public: false,
        is_serverless: true,
        ports: vec!["8080/http".to_string()],
        readme: String::new(),
        volume_mount_path: "/workspace".to_string(),
    })
    .expect("request should serialize");

    assert_eq!(
        payload,
        json!({
            "name": "lf-workspace-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "containerDiskInGb": 10,
            "env": {},
            "isPublic": false,
            "isServerless": true,
            "ports": ["8080/http"],
            "readme": "",
            "volumeMountPath": "/workspace"
        })
    );
}

#[test]
fn parses_template_response() {
    let response: RunPodTemplateResponse = serde_json::from_value(json!({
        "id": "template-1",
        "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
        "volumeMountPath": "/workspace",
        "isServerless": true,
        "ports": ["8080/http"]
    }))
    .expect("template response should parse");

    let observation = template_from_response(response).expect("template should map");

    assert_eq!(observation.id, "template-1");
    assert_eq!(observation.status, ProviderResourceStatus::Ready);
}

#[test]
fn rejects_non_serverless_template_response() {
    let response: RunPodTemplateResponse = serde_json::from_value(json!({
        "id": "template-1",
        "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
        "volumeMountPath": "/workspace",
        "isServerless": false
    }))
    .expect("template response should parse");

    let error = template_from_response(response).expect_err("pod template should be rejected");

    assert_eq!(error, ProviderClientError::ResponseInvalid);
}

#[test]
fn parses_endpoint_response_and_derives_invoke_url() {
    let response: RunPodEndpointResponse = serde_json::from_value(json!({
        "id": "endpoint-1",
        "status": "RUNNING",
        "gpuTypeIds": ["NVIDIA RTX 4090"],
        "dataCenterIds": ["EU-RO-1"]
    }))
    .expect("endpoint response should parse");

    let observation = endpoint_from_response(response).expect("endpoint should map");

    assert_eq!(observation.status, ProviderResourceStatus::Running);
    assert_eq!(
        observation.endpoint_invoke_url,
        "https://api.runpod.ai/v2/endpoint-1/run"
    );
}

#[test]
fn parses_endpoint_response_without_status_as_ready() {
    let response: RunPodEndpointResponse = serde_json::from_value(json!({
        "id": "endpoint-1",
        "gpuTypeIds": ["NVIDIA RTX 4090"],
        "dataCenterIds": ["EU-RO-1"],
        "idleTimeout": 5
    }))
    .expect("endpoint response should parse");

    let observation = endpoint_from_response(response).expect("endpoint should map");

    assert_eq!(observation.status, ProviderResourceStatus::Ready);
}

#[test]
fn maps_rest_status_codes_to_provider_errors() {
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::UNAUTHORIZED),
        Some(ProviderClientError::Unauthorized)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::NOT_FOUND),
        Some(ProviderClientError::NotFound)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::CONFLICT),
        Some(ProviderClientError::Conflict)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
        Some(ProviderClientError::RateLimited)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::BAD_REQUEST),
        Some(ProviderClientError::RequestRejected)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::UNPROCESSABLE_ENTITY),
        Some(ProviderClientError::RequestRejected)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::GATEWAY_TIMEOUT),
        Some(ProviderClientError::Indeterminate)
    );
    assert_eq!(
        provider_error_from_rest_status(reqwest::StatusCode::SERVICE_UNAVAILABLE),
        Some(ProviderClientError::ApiUnavailable)
    );
}

#[test]
fn rejects_invalid_rest_payloads() {
    let response: RunPodNetworkVolumeResponse = serde_json::from_value(json!({
        "id": "",
        "dataCenterId": "EU-RO-1",
        "size": 80
    }))
    .expect("payload should parse");

    let error = network_volume_from_response(response).expect_err("blank resource id should fail");

    assert_eq!(error, ProviderClientError::ResponseInvalid);
}
