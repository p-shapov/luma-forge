use crate::workspace_setup::WorkspaceSetupCatalogReader;
use serde_json::{json, Value};

use super::{
    error::BundledCatalogError, parser::parse_endpoint_profiles,
    parser::parse_provisioning_profiles, parser::parse_workflow_catalog,
    reader::BundledCatalogReader,
};

const COMMIT_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn reads_bundled_catalogs() {
    let reader = BundledCatalogReader;

    assert!(!reader
        .workflow_catalog()
        .expect("workflow catalog")
        .workflow_presets
        .is_empty());
    assert!(!reader
        .provisioning_profiles()
        .expect("provisioning profiles")
        .is_empty());
    assert!(!reader
        .endpoint_profiles()
        .expect("endpoint profiles")
        .is_empty());
}

#[test]
fn rejects_empty_workflow_catalog() {
    let error = parse_workflow_catalog(r#"{"id":"catalog","version":"1","workflow_presets":[]}"#)
        .expect_err("empty catalog should fail");

    assert_eq!(error, BundledCatalogError::ValidationFailed);
}

#[test]
fn rejects_workflow_catalog_with_missing_model_asset_install_path() {
    let error = parse_workflow_catalog(
        r#"
        {
          "id": "catalog",
          "version": "1",
          "workflow_presets": [
            {
              "id": "preset",
              "version": "1",
              "name": "Preset",
              "workflow_execution_type": "t2i",
              "required_base_volume_size_bytes": 1,
              "required_comfyui_source": {
                "source_type": "git",
                "repository_url": "https://example.test/comfyui.git",
                "revision": "0123456789abcdef0123456789abcdef01234567"
              },
              "required_model_assets": [
                {
	                  "id": "asset",
	                  "name": "Asset",
	                  "model_asset_kind": "checkpoint",
	                  "download_source": {
                    "source_type": "huggingface",
                    "repository_id": "owner/model",
                    "file_path": "model.safetensors",
                    "revision": "main"
                  }
                }
              ],
              "required_custom_nodes": []
            }
          ]
        }
        "#,
    )
    .expect_err("missing install path should fail");

    assert_eq!(error, BundledCatalogError::ParseFailed);
}

#[test]
fn rejects_workflow_catalog_with_unsafe_model_asset_install_path() {
    let error = parse_workflow_catalog(
        r#"
        {
          "id": "catalog",
          "version": "1",
          "workflow_presets": [
            {
              "id": "preset",
              "version": "1",
              "name": "Preset",
              "workflow_execution_type": "t2i",
              "required_base_volume_size_bytes": 1,
              "required_comfyui_source": {
                "source_type": "git",
                "repository_url": "https://example.test/comfyui.git",
                "revision": "0123456789abcdef0123456789abcdef01234567"
              },
              "required_model_assets": [
                {
	                  "id": "asset",
	                  "name": "Asset",
	                  "model_asset_kind": "checkpoint",
	                  "download_source": {
                    "source_type": "huggingface",
                    "repository_id": "owner/model",
                    "file_path": "model.safetensors",
                    "revision": "main"
                  },
                  "install": {
                    "comfyui_relative_path": "../model.safetensors"
                  }
                }
              ],
              "required_custom_nodes": []
            }
          ]
        }
        "#,
    )
    .expect_err("unsafe install path should fail");

    assert_eq!(error, BundledCatalogError::ValidationFailed);
}

#[test]
fn rejects_malformed_profiles() {
    let error = parse_provisioning_profiles("not json").expect_err("json should fail");

    assert_eq!(error, BundledCatalogError::ParseFailed);
}

#[test]
fn validates_workflow_catalog_source_and_custom_node_surface_fields() {
    let mut valid = valid_workflow_catalog();
    parse_workflow_catalog(&valid.to_string()).expect("valid workflow catalog");

    valid["workflow_presets"][0]["required_custom_nodes"] = json!([
        {
            "id": "node",
            "name": "Node",
            "git_source": {
                "source_type": "git",
                "repository_url": "https://example.test/node.git",
                "revision": COMMIT_REVISION
            },
            "install": {
                "comfyui_custom_nodes_relative_path": "custom_nodes/node"
            }
        }
    ]);
    parse_workflow_catalog(&valid.to_string()).expect("optional requirements path should pass");
    valid["workflow_presets"][0]["required_custom_nodes"][0]["install"]
        ["python_requirements_path"] = json!("requirements.txt");
    parse_workflow_catalog(&valid.to_string()).expect("safe requirements path should pass");
    let invalid_cases = [
        (
            "non-url comfyui repository",
            "/workflow_presets/0/required_comfyui_source/repository_url",
            json!("not a url"),
        ),
        (
            "blank comfyui revision",
            "/workflow_presets/0/required_comfyui_source/revision",
            json!(" "),
        ),
        (
            "mutable comfyui revision",
            "/workflow_presets/0/required_comfyui_source/revision",
            json!("main"),
        ),
        (
            "short comfyui revision",
            "/workflow_presets/0/required_comfyui_source/revision",
            json!("0123456"),
        ),
        (
            "malformed huggingface repository id",
            "/workflow_presets/0/required_model_assets/0/download_source/repository_id",
            json!("owner/model/extra"),
        ),
        (
            "unsafe huggingface file path",
            "/workflow_presets/0/required_model_assets/0/download_source/file_path",
            json!("../model.safetensors"),
        ),
        (
            "blank huggingface revision",
            "/workflow_presets/0/required_model_assets/0/download_source/revision",
            json!(""),
        ),
        (
            "custom node outside custom_nodes",
            "/workflow_presets/0/required_custom_nodes/0/install/comfyui_custom_nodes_relative_path",
            json!("models/node"),
        ),
        (
            "unsafe custom node checkout",
            "/workflow_presets/0/required_custom_nodes/0/install/comfyui_custom_nodes_relative_path",
            json!("custom_nodes/../node"),
        ),
        (
            "unsafe custom node requirements",
            "/workflow_presets/0/required_custom_nodes/0/install/python_requirements_path",
            json!("../requirements.txt"),
        ),
        (
            "blank custom node requirements",
            "/workflow_presets/0/required_custom_nodes/0/install/python_requirements_path",
            json!(" "),
        ),
        (
            "blank custom node repository",
            "/workflow_presets/0/required_custom_nodes/0/git_source/repository_url",
            json!(""),
        ),
        (
            "mutable custom node revision",
            "/workflow_presets/0/required_custom_nodes/0/git_source/revision",
            json!("main"),
        ),
    ];

    for (name, pointer, replacement) in invalid_cases {
        let mut catalog = valid.clone();
        *catalog.pointer_mut(pointer).expect(name) = replacement;
        let error = parse_workflow_catalog(&catalog.to_string()).expect_err(name);
        assert_eq!(error, BundledCatalogError::ValidationFailed, "{name}");
    }
}

#[test]
fn validates_provisioning_profile_surface_fields() {
    parse_provisioning_profiles(&valid_provisioning_profiles().to_string())
        .expect("valid provisioning profiles");

    let invalid_cases = [
        (
            "malformed docker ref",
            "/0/provisioner_worker_runtime/docker_image_ref",
            json!("bad ref"),
        ),
        (
            "separator-only docker ref",
            "/0/provisioner_worker_runtime/docker_image_ref",
            json!("///"),
        ),
        (
            "trailing slash docker ref",
            "/0/provisioner_worker_runtime/docker_image_ref",
            json!("repo/"),
        ),
        (
            "tag-only docker ref",
            "/0/provisioner_worker_runtime/docker_image_ref",
            json!(":tag"),
        ),
        (
            "relative worker mount",
            "/0/provisioner_worker_runtime/volume_mount_path",
            json!("workspace"),
        ),
        (
            "root worker mount",
            "/0/provisioner_worker_runtime/volume_mount_path",
            json!("/"),
        ),
        (
            "malformed status path",
            "/0/provisioner_worker_runtime/status_endpoint/status_path",
            json!("/status?debug=true"),
        ),
        (
            "invalid status port",
            "/0/provisioner_worker_runtime/status_endpoint/port",
            json!(0),
        ),
        (
            "unsupported cloud type",
            "/0/gpu_cloud_provider_config/cloud_type",
            json!("private"),
        ),
        (
            "status port not exposed",
            "/0/gpu_cloud_provider_config/expose_http_ports",
            json!([9000]),
        ),
    ];

    for (name, pointer, replacement) in invalid_cases {
        let mut profiles = valid_provisioning_profiles();
        *profiles.pointer_mut(pointer).expect(name) = replacement;
        let error = parse_provisioning_profiles(&profiles.to_string()).expect_err(name);
        assert_eq!(error, BundledCatalogError::ValidationFailed, "{name}");
    }
}

#[test]
fn validates_endpoint_profile_surface_fields() {
    parse_endpoint_profiles(&valid_endpoint_profiles().to_string())
        .expect("valid endpoint profiles");

    let invalid_cases = [
        (
            "malformed docker ref",
            "/0/endpoint_worker_runtime/docker_image_ref",
            json!("bad ref"),
        ),
        (
            "separator-only docker ref",
            "/0/endpoint_worker_runtime/docker_image_ref",
            json!("///"),
        ),
        (
            "trailing slash docker ref",
            "/0/endpoint_worker_runtime/docker_image_ref",
            json!("repo/"),
        ),
        (
            "tag-only docker ref",
            "/0/endpoint_worker_runtime/docker_image_ref",
            json!(":tag"),
        ),
        (
            "unsafe runpod mount",
            "/0/gpu_cloud_provider_config/volume_mount_path",
            json!("/workspace/../other"),
        ),
        (
            "root runpod mount",
            "/0/gpu_cloud_provider_config/volume_mount_path",
            json!("/"),
        ),
        (
            "malformed health path",
            "/0/endpoint_worker_runtime/health_path",
            json!("health"),
        ),
        (
            "malformed invoke path",
            "/0/endpoint_worker_runtime/invoke_path",
            json!("/prompt#fragment"),
        ),
        (
            "invalid worker port",
            "/0/endpoint_worker_runtime/http_port",
            json!(0),
        ),
        (
            "unsupported scaler type",
            "/0/gpu_cloud_provider_config/scaling/scaler_type",
            json!("custom"),
        ),
        (
            "inconsistent scaling",
            "/0/gpu_cloud_provider_config/scaling/min_workers",
            json!(2),
        ),
    ];

    for (name, pointer, replacement) in invalid_cases {
        let mut profiles = valid_endpoint_profiles();
        *profiles.pointer_mut(pointer).expect(name) = replacement;
        let error = parse_endpoint_profiles(&profiles.to_string()).expect_err(name);
        assert_eq!(error, BundledCatalogError::ValidationFailed, "{name}");
    }
}

fn valid_workflow_catalog() -> Value {
    json!({
        "id": "catalog",
        "version": "1",
        "workflow_presets": [
            {
                "id": "preset",
                "version": "1",
                "name": "Preset",
                "workflow_execution_type": "t2i",
                "required_base_volume_size_bytes": 1,
                "required_comfyui_source": {
                    "source_type": "git",
                    "repository_url": "https://example.test/comfyui.git",
                    "revision": COMMIT_REVISION
                },
                "required_model_assets": [
                    {
                        "id": "asset",
                        "name": "Asset",
                        "model_asset_kind": "checkpoint",
                        "download_source": {
                            "source_type": "huggingface",
                            "repository_id": "owner/model",
                            "file_path": "subdir/model.safetensors",
                            "revision": "main"
                        },
                        "install": {
                            "comfyui_relative_path": "models/checkpoints/model.safetensors"
                        }
                    }
                ],
                "required_custom_nodes": []
            }
        ]
    })
}

fn valid_provisioning_profiles() -> Value {
    json!([
        {
            "gpu_cloud_provider_id": "runpod",
            "id": "profile",
            "version": "1",
            "name": "Profile",
            "provisioner_worker_runtime": {
                "provisioner_version": "1",
                "docker_image_ref": "ghcr.io/luma-forge/provisioner:1.0.0",
                "volume_mount_path": "/workspace",
                "container_disk_bytes": 1,
                "compute_type": "pod",
                "status_endpoint": {
                    "port": 8000,
                    "protocol": "http",
                    "status_path": "/status"
                }
            },
            "gpu_cloud_provider_config": {
                "cloud_type": "secure",
                "pod_template_id": null,
                "network_volume_mount_path": "/workspace",
                "expose_http_ports": [8000],
                "env": {
                    "LUMA_FORGE": "1"
                }
            }
        }
    ])
}

fn valid_endpoint_profiles() -> Value {
    json!([
        {
            "gpu_cloud_provider_id": "runpod",
            "id": "endpoint",
            "version": "1",
            "name": "Endpoint",
            "workflow_execution_type": "t2i",
            "endpoint_worker_runtime": {
                "endpoint_worker_version": "1",
                "docker_image_ref": "ghcr.io/luma-forge/endpoint-worker:1.0.0",
                "http_port": 8188,
                "health_path": "/health",
                "invoke_path": "/prompt"
            },
            "gpu_cloud_provider_config": {
                "endpoint_template_id": null,
                "container_disk_bytes": 1,
                "volume_mount_path": "/workspace",
                "env": {
                    "LUMA_FORGE": "1"
                },
                "scaling": {
                    "min_workers": 0,
                    "max_workers": 1,
                    "idle_timeout_seconds": 300,
                    "scaler_type": "queue_delay",
                    "scaler_value": 4
                }
            }
        }
    ])
}
