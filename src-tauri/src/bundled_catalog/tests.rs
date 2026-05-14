use crate::workspace_setup::WorkspaceSetupCatalogReader;
use serde_json::{json, Value};

use super::{
    error::BundledCatalogError, parser::parse_workflow_catalog, reader::BundledCatalogReader,
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
