use crate::workspace::workspace_setup_service::WorkspaceSetupCatalogReader;

use super::{
    bundled_catalog_error::BundledCatalogError,
    bundled_catalog_parser::parse_provisioning_profiles,
    bundled_catalog_parser::parse_workflow_catalog, bundled_catalog_reader::BundledCatalogReader,
};

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
                "revision": "main"
              },
              "required_model_assets": [
                {
                  "id": "asset",
                  "name": "Asset",
                  "model_asset_kind": "checkpoint",
                  "file_size_bytes": 1,
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
                "revision": "main"
              },
              "required_model_assets": [
                {
                  "id": "asset",
                  "name": "Asset",
                  "model_asset_kind": "checkpoint",
                  "file_size_bytes": 1,
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
