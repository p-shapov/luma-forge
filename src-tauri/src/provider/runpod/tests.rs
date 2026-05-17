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
            endpoints_by_name,
            mapper::{
                endpoint_from_response, identity_from_graphql_response,
                inventory_from_graphql_response, network_volume_from_response,
                pod_from_response_with_context, template_from_response, RunPodPodResponseContext,
            },
            network_volumes_by_name, pods_by_name_and_volume, provider_error_from_inventory_status,
            provider_error_from_rest_status, templates_by_name, RunPodClient,
            RunPodFindEndpointInput, RUNPOD_REST_ENDPOINT,
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

fn template_env() -> HashMap<String, String> {
    HashMap::new()
}

fn template_env_json() -> serde_json::Value {
    json!(template_env())
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
fn network_volume_discovery_filters_by_name_datacenter_and_size() {
    let payloads: Vec<RunPodNetworkVolumeResponse> = serde_json::from_value(json!([
        {
            "id": "volume-1",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80
        },
        {
            "id": "volume-wrong-size",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "EU-RO-1",
            "size": 100
        },
        {
            "id": "volume-wrong-name",
            "name": "other-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80
        }
    ]))
    .expect("volume list should parse");

    let observations =
        network_volumes_by_name(payloads, "luma-forge-workspace-1-volume", "EU-RO-1", 80)
            .expect("matching volumes should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "volume-1");
    assert_eq!(
        observations[0].status,
        ProviderResourceStatus::Creating,
        "list responses without status must be refreshed before use"
    );
}

#[test]
fn network_volume_discovery_preserves_explicit_status() {
    let payloads: Vec<RunPodNetworkVolumeResponse> = serde_json::from_value(json!([
        {
            "id": "volume-1",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80,
            "status": "READY"
        }
    ]))
    .expect("volume list should parse");

    let observations =
        network_volumes_by_name(payloads, "luma-forge-workspace-1-volume", "EU-RO-1", 80)
            .expect("matching volumes should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].status, ProviderResourceStatus::Ready);
}

#[test]
fn network_volume_discovery_returns_multiple_matches_when_provider_has_duplicates() {
    let payloads: Vec<RunPodNetworkVolumeResponse> = serde_json::from_value(json!([
        {
            "id": "volume-1",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80
        },
        {
            "id": "volume-2",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "EU-RO-1",
            "size": 80
        }
    ]))
    .expect("volume list should parse");

    let observations =
        network_volumes_by_name(payloads, "luma-forge-workspace-1-volume", "EU-RO-1", 80)
            .expect("matching volumes should map");

    assert_eq!(observations.len(), 2);
}

#[test]
fn network_volume_discovery_returns_zero_without_required_match() {
    let payloads: Vec<RunPodNetworkVolumeResponse> = serde_json::from_value(json!([
        {
            "id": "volume-1",
            "name": "luma-forge-workspace-1-volume",
            "dataCenterId": "US-KS-1",
            "size": 80
        }
    ]))
    .expect("volume list should parse");

    let observations =
        network_volumes_by_name(payloads, "luma-forge-workspace-1-volume", "EU-RO-1", 80)
            .expect("matching volumes should map");

    assert!(observations.is_empty());
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
        ports: vec!["8000/http".to_string()],
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
            "ports": ["8000/http"]
        })
    );
}

#[test]
fn parses_pod_response_and_derives_http_proxy_status_url() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "image": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
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

    let observation = pod_from_response_with_context(response, None).expect("pod should map");

    assert_eq!(observation.data_center_id, "EU-RO-1");
    assert_eq!(observation.selected_gpu_id, "NVIDIA GeForce RTX 4090");
    assert_eq!(
        observation.image_name,
        "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111"
    );
    assert_eq!(observation.status, ProviderResourceStatus::Running);
    assert_eq!(
        observation.provisioner_status_url.as_deref(),
        Some("https://pod-1-8080.proxy.runpod.net/status")
    );
}

#[test]
fn parses_legacy_pod_response_image_name_field() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "desiredStatus": "RUNNING",
        "machine": {
            "dataCenterId": "EU-RO-1",
            "gpuTypeId": "NVIDIA GeForce RTX 4090"
        },
        "ports": ["8080/http"]
    }))
    .expect("pod response should parse");

    let observation = pod_from_response_with_context(response, None).expect("pod should map");

    assert_eq!(
        observation.image_name,
        "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111"
    );
}

#[test]
fn rejects_pod_response_without_image_identity() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "desiredStatus": "RUNNING",
        "machine": {
            "dataCenterId": "EU-RO-1",
            "gpuTypeId": "NVIDIA GeForce RTX 4090"
        },
        "ports": ["8080/http"]
    }))
    .expect("pod response should parse");

    let error =
        pod_from_response_with_context(response, None).expect_err("image identity is required");

    assert_eq!(error, ProviderClientError::ResponseInvalid);
}

#[test]
fn parses_pod_response_with_request_context_when_provider_omits_placement_fields() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "desiredStatus": "RUNNING",
        "publicIp": "",
        "ports": ["8000/http"],
        "portMappings": null,
        "machine": {}
    }))
    .expect("pod response should parse");

    let observation = pod_from_response_with_context(
        response,
        Some(RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA GeForce RTX 4090".to_string(),
        }),
    )
    .expect("pod should map with request context");

    assert_eq!(observation.data_center_id, "EU-RO-1");
    assert_eq!(observation.selected_gpu_id, "NVIDIA GeForce RTX 4090");
    assert_eq!(
        observation.provisioner_status_url.as_deref(),
        Some("https://pod-1-8000.proxy.runpod.net/status")
    );
}

#[test]
fn parses_direct_tcp_pod_response_with_public_ip_and_port_mapping() {
    let response: RunPodPodResponse = serde_json::from_value(json!({
        "id": "pod-1",
        "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
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
            "8080/tcp"
        ]
    }))
    .expect("pod response should parse");

    let observation = pod_from_response_with_context(response, None).expect("pod should map");

    assert_eq!(
        observation.provisioner_status_url.as_deref(),
        Some("http://203.0.113.10:30001/status")
    );
}

#[test]
fn filters_pods_by_name_and_volume_for_discovery() {
    let payloads: Vec<RunPodPodResponse> = serde_json::from_value(json!([
        {
            "id": "pod-1",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        },
        {
            "id": "pod-wrong-volume",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "other-volume",
            "ports": ["8080/http"]
        },
        {
            "id": "pod-terminated",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "TERMINATED",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        }
    ]))
    .expect("pod list should parse");

    let observations = pods_by_name_and_volume(
        payloads,
        "luma-forge-workspace-1-provisioner",
        "volume-1",
        RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        },
    )
    .expect("matching pods should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "pod-1");
    assert_eq!(
        observations[0].provisioner_status_url.as_deref(),
        Some("https://pod-1-8080.proxy.runpod.net/status")
    );
}

#[test]
fn pod_discovery_returns_candidate_with_mismatched_image_identity() {
    let payloads: Vec<RunPodPodResponse> = serde_json::from_value(json!([
        {
            "id": "pod-1",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:9999999999999999999999999999999999999999999999999999999999999999",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        }
    ]))
    .expect("pod list should parse");

    let observations = pods_by_name_and_volume(
        payloads,
        "luma-forge-workspace-1-provisioner",
        "volume-1",
        RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        },
    )
    .expect("matching pods should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "pod-1");
    assert_eq!(
        observations[0].image_name,
        "ghcr.io/luma-forge/provisioner@sha256:9999999999999999999999999999999999999999999999999999999999999999"
    );
}

#[test]
fn pod_discovery_filter_returns_multiple_matches_when_provider_has_duplicates() {
    let payloads: Vec<RunPodPodResponse> = serde_json::from_value(json!([
        {
            "id": "pod-1",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        },
        {
            "id": "pod-2",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        }
    ]))
    .expect("pod list should parse");

    let observations = pods_by_name_and_volume(
        payloads,
        "luma-forge-workspace-1-provisioner",
        "volume-1",
        RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        },
    )
    .expect("matching pods should map");

    assert_eq!(observations.len(), 2);
}

#[test]
fn pod_discovery_filter_returns_zero_matches_without_name_and_volume_match() {
    let payloads: Vec<RunPodPodResponse> = serde_json::from_value(json!([
        {
            "id": "pod-1",
            "name": "luma-forge-workspace-1-provisioner",
            "imageName": "ghcr.io/luma-forge/provisioner@sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "other-volume",
            "ports": ["8080/http"]
        }
    ]))
    .expect("pod list should parse");

    let observations = pods_by_name_and_volume(
        payloads,
        "luma-forge-workspace-1-provisioner",
        "volume-1",
        RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        },
    )
    .expect("matching pods should map");

    assert!(observations.is_empty());
}

#[test]
fn pod_discovery_rejects_candidate_without_image_identity() {
    let payloads: Vec<RunPodPodResponse> = serde_json::from_value(json!([
        {
            "id": "pod-1",
            "name": "luma-forge-workspace-1-provisioner",
            "desiredStatus": "RUNNING",
            "networkVolumeId": "volume-1",
            "ports": ["8080/http"]
        }
    ]))
    .expect("pod list should parse");

    let error = pods_by_name_and_volume(
        payloads,
        "luma-forge-workspace-1-provisioner",
        "volume-1",
        RunPodPodResponseContext {
            data_center_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA RTX 4090".to_string(),
        },
    )
    .expect_err("image identity is required");

    assert_eq!(error, ProviderClientError::ResponseInvalid);
}

#[test]
fn serializes_serverless_template_create_request_with_comfyui_http_port() {
    let endpoint_ref = "ghcr.io/luma-forge/endpoint-worker:dev".to_string();
    let payload = serde_json::to_value(RunPodCreateTemplateRequest {
        name: "lf-workspace-endpoint-template".to_string(),
        image_name: endpoint_ref.clone(),
        container_disk_in_gb: 10,
        env: HashMap::new(),
        is_public: false,
        is_serverless: true,
        ports: vec!["8188/http".to_string()],
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
            "ports": ["8188/http"],
            "readme": "",
            "volumeMountPath": "/workspace"
        })
    );
}

#[test]
fn parses_template_response() {
    let response: RunPodTemplateResponse = serde_json::from_value(json!({
        "id": "template-1",
        "name": "lf-workspace-endpoint-template",
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
fn template_discovery_filters_by_name_image_port_and_mount_path() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8080/http"]
        },
        {
            "id": "template-wrong-port",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8000/http"]
        },
        {
            "id": "template-not-serverless",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": false,
            "ports": ["8080/http"]
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:dev",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "template-1");
}

#[test]
fn template_discovery_returns_multiple_matches_when_provider_has_duplicates() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8080/http"]
        },
        {
            "id": "template-2",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8080/http"]
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:dev",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert_eq!(observations.len(), 2);
}

#[test]
fn template_discovery_returns_zero_without_required_match() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8080/http"]
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:other",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert!(observations.is_empty());
}

#[test]
fn template_discovery_accepts_missing_runtime_env_when_no_env_is_expected() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "volumeMountPath": "/workspace",
            "isServerless": true,
            "ports": ["8080/http"]
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:dev",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert_eq!(observations.len(), 1);
}

#[test]
fn template_discovery_rejects_missing_serverless_flag() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "ports": ["8080/http"]
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:dev",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert!(observations.is_empty());
}

#[test]
fn template_discovery_rejects_missing_ports() {
    let payloads: Vec<RunPodTemplateResponse> = serde_json::from_value(json!([
        {
            "id": "template-1",
            "name": "luma-forge-workspace-1-endpoint-template",
            "imageName": "ghcr.io/luma-forge/endpoint-worker:dev",
            "env": template_env_json(),
            "volumeMountPath": "/workspace",
            "isServerless": true
        }
    ]))
    .expect("template list should parse");

    let observations = templates_by_name(
        payloads,
        "luma-forge-workspace-1-endpoint-template",
        "ghcr.io/luma-forge/endpoint-worker:dev",
        &template_env(),
        8080,
        "/workspace",
    )
    .expect("matching templates should map");

    assert!(observations.is_empty());
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
fn parses_endpoint_response_with_comma_separated_datacenter_ids() {
    let response: RunPodEndpointResponse = serde_json::from_value(json!({
        "id": "endpoint-1",
        "status": "RUNNING",
        "gpuTypeIds": ["NVIDIA RTX 4090"],
        "dataCenterIds": "EU-RO-1, US-KS-1"
    }))
    .expect("endpoint response should parse");

    let observation = endpoint_from_response(response).expect("endpoint should map");

    assert_eq!(observation.data_center_id, "EU-RO-1");
}

#[test]
fn parses_endpoint_response_without_status_as_ready() {
    let response: RunPodEndpointResponse = serde_json::from_value(json!({
        "id": "endpoint-1",
        "name": "lf-workspace-endpoint",
        "gpuTypeIds": ["NVIDIA RTX 4090"],
        "dataCenterIds": ["EU-RO-1"],
        "idleTimeout": 5
    }))
    .expect("endpoint response should parse");

    let observation = endpoint_from_response(response).expect("endpoint should map");

    assert_eq!(observation.status, ProviderResourceStatus::Ready);
}

#[test]
fn endpoint_discovery_filters_by_name_resources_and_placement() {
    let payloads: Vec<RunPodEndpointResponse> = serde_json::from_value(json!([
        {
            "id": "endpoint-1",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 5
        },
        {
            "id": "endpoint-wrong-volume",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-2",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 5
        },
        {
            "id": "endpoint-wrong-gpu",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 3090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 5
        }
    ]))
    .expect("endpoint list should parse");

    let observations = endpoints_by_name(
        payloads,
        &RunPodFindEndpointInput {
            name: "luma-forge-workspace-1-endpoint",
            template_id: "template-1",
            network_volume_id: "volume-1",
            data_center_id: "EU-RO-1",
            selected_gpu_id: "NVIDIA RTX 4090",
            idle_timeout: 5,
        },
    )
    .expect("matching endpoints should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "endpoint-1");
}

#[test]
fn endpoint_discovery_accepts_comma_separated_datacenter_ids() {
    let payloads: Vec<RunPodEndpointResponse> = serde_json::from_value(json!([
        {
            "id": "endpoint-1",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": "US-KS-1, EU-RO-1",
            "idleTimeout": 5
        }
    ]))
    .expect("endpoint list should parse");

    let observations = endpoints_by_name(
        payloads,
        &RunPodFindEndpointInput {
            name: "luma-forge-workspace-1-endpoint",
            template_id: "template-1",
            network_volume_id: "volume-1",
            data_center_id: "EU-RO-1",
            selected_gpu_id: "NVIDIA RTX 4090",
            idle_timeout: 5,
        },
    )
    .expect("matching endpoints should map");

    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].id, "endpoint-1");
}

#[test]
fn endpoint_discovery_returns_multiple_matches_when_provider_has_duplicates() {
    let payloads: Vec<RunPodEndpointResponse> = serde_json::from_value(json!([
        {
            "id": "endpoint-1",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 5
        },
        {
            "id": "endpoint-2",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 5
        }
    ]))
    .expect("endpoint list should parse");

    let observations = endpoints_by_name(
        payloads,
        &RunPodFindEndpointInput {
            name: "luma-forge-workspace-1-endpoint",
            template_id: "template-1",
            network_volume_id: "volume-1",
            data_center_id: "EU-RO-1",
            selected_gpu_id: "NVIDIA RTX 4090",
            idle_timeout: 5,
        },
    )
    .expect("matching endpoints should map");

    assert_eq!(observations.len(), 2);
}

#[test]
fn endpoint_discovery_returns_zero_without_required_match() {
    let payloads: Vec<RunPodEndpointResponse> = serde_json::from_value(json!([
        {
            "id": "endpoint-1",
            "name": "luma-forge-workspace-1-endpoint",
            "templateId": "template-1",
            "networkVolumeId": "volume-1",
            "gpuTypeIds": ["NVIDIA RTX 4090"],
            "dataCenterIds": ["EU-RO-1"],
            "idleTimeout": 10
        }
    ]))
    .expect("endpoint list should parse");

    let observations = endpoints_by_name(
        payloads,
        &RunPodFindEndpointInput {
            name: "luma-forge-workspace-1-endpoint",
            template_id: "template-1",
            network_volume_id: "volume-1",
            data_center_id: "EU-RO-1",
            selected_gpu_id: "NVIDIA RTX 4090",
            idle_timeout: 5,
        },
    )
    .expect("matching endpoints should map");

    assert!(observations.is_empty());
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
