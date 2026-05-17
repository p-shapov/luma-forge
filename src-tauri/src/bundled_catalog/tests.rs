use crate::workspace_setup::WorkspaceSetupCatalogReader;
use serde_json::{json, Value};

use super::{
    error::BundledCatalogError,
    parser::{parse_runtime_catalog, parse_workflow_catalog},
    reader::BundledCatalogReader,
};

const COMMIT_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn reads_bundled_catalogs() {
    let reader = BundledCatalogReader;

    assert!(!reader
        .runtime_catalog()
        .expect("runtime catalog")
        .contracts
        .is_empty());
    assert!(!reader
        .workflow_catalog()
        .expect("workflow catalog")
        .workflow_presets
        .is_empty());
}

#[test]
fn validates_runtime_catalog_surface_fields() {
    let valid = valid_runtime_catalog();
    let parsed = parse_runtime_catalog(&valid.to_string()).expect("valid runtime catalog");
    let resolved = parsed
        .resolve("comfyui-python312-cu121", "1.0.0")
        .expect("runtime snapshot");
    assert_eq!(resolved.contract_id, "comfyui-python312-cu121");
    assert_eq!(resolved.contract_version, "1.0.0");

    let invalid_cases = [
        (
            "empty contracts",
            "/contracts",
            json!([]),
            BundledCatalogError::ValidationFailed,
        ),
        (
            "malformed contract id",
            "/contracts/0/id",
            json!("ComfyUI"),
            BundledCatalogError::ValidationFailed,
        ),
        (
            "malformed version",
            "/contracts/0/revisions/0/version",
            json!("1"),
            BundledCatalogError::ValidationFailed,
        ),
        (
            "mutable provisioner image",
            "/contracts/0/revisions/0/provisioner_image_ref",
            json!("ghcr.io/luma-forge/provisioner-worker:latest"),
            BundledCatalogError::ValidationFailed,
        ),
    ];

    for (name, pointer, replacement, expected) in invalid_cases {
        let mut catalog = valid.clone();
        *catalog.pointer_mut(pointer).expect(name) = replacement;
        let error = parse_runtime_catalog(&catalog.to_string()).expect_err(name);
        assert_eq!(error, expected, "{name}");
    }

    let mut duplicate = valid.clone();
    duplicate["contracts"] = json!([
        duplicate["contracts"][0].clone(),
        duplicate["contracts"][0].clone()
    ]);
    let error = parse_runtime_catalog(&duplicate.to_string()).expect_err("duplicate contract id");
    assert_eq!(error, BundledCatalogError::ValidationFailed);
}

#[test]
fn rejects_empty_workflow_catalog() {
    let runtime_catalog = runtime_catalog();
    let error = parse_workflow_catalog(r#"{"workflow_presets":[]}"#, &runtime_catalog)
        .expect_err("empty catalog should fail");

    assert_eq!(error, BundledCatalogError::ValidationFailed);
}

#[test]
fn rejects_workflow_catalog_with_missing_model_asset_install_path() {
    let runtime_catalog = runtime_catalog();
    let mut catalog = valid_workflow_catalog();
    catalog["workflow_presets"][0]["required_model_assets"][0]
        .as_object_mut()
        .expect("asset object")
        .remove("install");

    let error = parse_workflow_catalog(&catalog.to_string(), &runtime_catalog)
        .expect_err("missing install path should fail");

    assert_eq!(error, BundledCatalogError::ParseFailed);
}

#[test]
fn validates_workflow_catalog_source_and_custom_node_surface_fields() {
    let runtime_catalog = runtime_catalog();
    let mut valid = valid_workflow_catalog();
    parse_workflow_catalog(&valid.to_string(), &runtime_catalog).expect("valid workflow catalog");

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
    parse_workflow_catalog(&valid.to_string(), &runtime_catalog)
        .expect("optional requirements path should pass");
    valid["workflow_presets"][0]["required_custom_nodes"][0]["install"]
        ["python_requirements_path"] = json!("requirements.txt");
    parse_workflow_catalog(&valid.to_string(), &runtime_catalog)
        .expect("safe requirements path should pass");
    let invalid_cases = [
        (
            "unknown runtime contract id",
            "/workflow_presets/0/runtime_contract/id",
            json!("unknown-runtime"),
        ),
        (
            "unknown runtime contract version",
            "/workflow_presets/0/runtime_contract/version",
            json!("9.0.0"),
        ),
        (
            "malformed runtime contract version",
            "/workflow_presets/0/runtime_contract/version",
            json!("1"),
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
        let error = parse_workflow_catalog(&catalog.to_string(), &runtime_catalog).expect_err(name);
        assert_eq!(error, BundledCatalogError::ValidationFailed, "{name}");
    }
}

fn runtime_catalog() -> crate::domain::runtime::RuntimeCatalog {
    parse_runtime_catalog(&valid_runtime_catalog().to_string()).expect("runtime catalog")
}

fn valid_runtime_catalog() -> Value {
    json!({
        "contracts": [
            {
                "id": "comfyui-python312-cu121",
                "revisions": [
                    {
                        "version": "1.0.0",
                        "provisioner_image_ref": "ghcr.io/luma-forge/provisioner-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111",
                        "endpoint_image_ref": "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222"
                    }
                ]
            }
        ]
    })
}

fn valid_workflow_catalog() -> Value {
    json!({
        "workflow_presets": [
            {
                "id": "preset",
                "version": "1",
                "name": "Preset",
                "workflow_execution_type": "t2i",
                "required_base_volume_size_bytes": 1,
                "runtime_contract": {
                    "id": "comfyui-python312-cu121",
                    "version": "1.0.0"
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
