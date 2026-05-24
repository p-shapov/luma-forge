use crate::{
    bundled_catalog::error::BundledCatalogError,
    domain::{
        provisioner::{validator::validate_provisioner_catalog, ProvisionerCatalog},
        runtime::{validator::validate_runtime_catalog, RuntimeCatalog},
        workflow::validator::validate_workflow_catalog,
        workflow::WorkflowCatalog,
    },
};

pub(super) fn parse_runtime_catalog(value: &str) -> Result<RuntimeCatalog, BundledCatalogError> {
    let catalog: RuntimeCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_runtime_catalog(&catalog).map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}

pub(super) fn parse_provisioner_catalog(
    value: &str,
) -> Result<ProvisionerCatalog, BundledCatalogError> {
    let catalog: ProvisionerCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_provisioner_catalog(&catalog).map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}

pub(super) fn parse_workflow_catalog(
    value: &str,
    runtime_catalog: &RuntimeCatalog,
    provisioner_catalog: &ProvisionerCatalog,
) -> Result<WorkflowCatalog, BundledCatalogError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_workflow_catalog(&catalog, runtime_catalog, provisioner_catalog)
        .map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn valid_runtime_catalog_json() -> String {
        format!(
            r#"{{
                "contracts": [
                    {{
                        "id": "comfyui-hidream-o1-dev",
                        "revisions": [
                            {{
                                "version": "1.0.0",
                                "endpoint_image_ref": "ghcr.io/example/endpoint@sha256:{DIGEST_B}"
                            }}
                        ]
                    }}
                ]
            }}"#
        )
    }

    fn valid_runtime_catalog() -> RuntimeCatalog {
        parse_runtime_catalog(&valid_runtime_catalog_json())
            .expect("valid runtime catalog should parse")
    }

    fn valid_provisioner_catalog_json() -> String {
        format!(
            r#"{{
                "contracts": [
                    {{
                        "id": "luma-forge-provisioner",
                        "revisions": [
                            {{
                                "version": "1.0.0",
                                "provisioner_worker_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_C}",
                                "volume_mount_path": "/workspace"
                            }}
                        ]
                    }}
                ]
            }}"#
        )
    }

    fn valid_provisioner_catalog() -> ProvisionerCatalog {
        parse_provisioner_catalog(&valid_provisioner_catalog_json())
            .expect("valid provisioner catalog should parse")
    }

    fn valid_workflow_catalog_json() -> &'static str {
        r#"{
            "workflow_presets": [
                {
                    "id": "comfyui-hidream-o1-dev",
                    "version": "1.0.0",
                    "name": "ComfyUI Text to Image Basic",
                    "workflow_execution_type": "t2i",
                    "required_base_volume_size_bytes": 85899345920,
                    "requires_hugging_face_api_key": true,
                    "runtime_contract": {
                        "id": "comfyui-hidream-o1-dev",
                        "version": "1.0.0"
                    },
                    "provisioner_contract": {
                        "id": "luma-forge-provisioner",
                        "version": "1.0.0"
                    },
                    "required_model_assets": [
                        {
                            "id": "hidream-o1-dev-checkpoint",
                            "name": "HiDream O1 Dev",
                            "download_source": {
                                "source_type": "huggingface",
                                "repository_id": "Comfy-Org/HiDream-O1-Image",
                                "file_path": "hidream_o1_image_dev_fp8_scaled.safetensors",
                                "revision": "462165984030d82259a11f4367a4eed129e94a7b"
                            },
                            "install_comfyui_relative_path": "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors"
                        }
                    ]
                }
            ]
        }"#
    }

    #[test]
    fn parse_runtime_catalog_accepts_valid_catalog() {
        let catalog = parse_runtime_catalog(&valid_runtime_catalog_json())
            .expect("valid runtime catalog should parse");

        let [contract] = catalog.contracts.as_slice() else {
            panic!("expected one runtime contract");
        };
        assert_eq!(contract.id, "comfyui-hidream-o1-dev");

        let [revision] = contract.revisions.as_slice() else {
            panic!("expected one runtime revision");
        };
        assert_eq!(revision.version, "1.0.0");
        assert!(revision
            .endpoint_image_ref
            .ends_with(&format!("@sha256:{DIGEST_B}")));
    }

    #[test]
    fn parse_runtime_catalog_maps_invalid_json_to_parse_failed() {
        let err = parse_runtime_catalog("{ invalid json")
            .expect_err("invalid JSON should fail before validation");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_runtime_catalog_maps_invalid_catalog_to_validation_failed() {
        let err = parse_runtime_catalog(r#"{"contracts":[]}"#)
            .expect_err("invalid catalog should fail validation");

        assert_eq!(err, BundledCatalogError::ValidationFailed);
    }

    #[test]
    fn parse_provisioner_catalog_accepts_valid_catalog() {
        let catalog = parse_provisioner_catalog(&valid_provisioner_catalog_json())
            .expect("valid provisioner catalog should parse");

        let [contract] = catalog.contracts.as_slice() else {
            panic!("expected one provisioner contract");
        };
        assert_eq!(contract.id, "luma-forge-provisioner");

        let [revision] = contract.revisions.as_slice() else {
            panic!("expected one provisioner revision");
        };
        assert_eq!(revision.version, "1.0.0");
        assert_eq!(revision.volume_mount_path, "/workspace");
        assert!(revision
            .provisioner_worker_image_ref
            .ends_with(&format!("@sha256:{DIGEST_C}")));
    }

    #[test]
    fn parse_provisioner_catalog_maps_invalid_json_to_parse_failed() {
        let err = parse_provisioner_catalog("{ invalid json")
            .expect_err("invalid JSON should fail before validation");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_provisioner_catalog_maps_invalid_catalog_to_validation_failed() {
        let err = parse_provisioner_catalog(r#"{"contracts":[]}"#)
            .expect_err("invalid catalog should fail validation");

        assert_eq!(err, BundledCatalogError::ValidationFailed);
    }

    #[test]
    fn parse_workflow_catalog_accepts_valid_catalog_without_custom_nodes() {
        let runtime_catalog = valid_runtime_catalog();
        let provisioner_catalog = valid_provisioner_catalog();
        let catalog = parse_workflow_catalog(
            valid_workflow_catalog_json(),
            &runtime_catalog,
            &provisioner_catalog,
        )
        .expect("valid workflow catalog should parse");

        let [preset] = catalog.workflow_presets.as_slice() else {
            panic!("expected one workflow preset");
        };
        assert_eq!(preset.id, "comfyui-hidream-o1-dev");
        assert_eq!(preset.required_model_assets.len(), 1);
    }

    #[test]
    fn parse_workflow_catalog_maps_invalid_json_to_parse_failed() {
        let runtime_catalog = valid_runtime_catalog();
        let provisioner_catalog = valid_provisioner_catalog();
        let err = parse_workflow_catalog("{ invalid json", &runtime_catalog, &provisioner_catalog)
            .expect_err("invalid JSON should fail before validation");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_workflow_catalog_requires_hugging_face_auth_flag() {
        let runtime_catalog = valid_runtime_catalog();
        let provisioner_catalog = valid_provisioner_catalog();
        let err = parse_workflow_catalog(
            r#"{
                "workflow_presets": [
                    {
                        "id": "comfyui-hidream-o1-dev",
                        "version": "1.0.0",
                        "name": "ComfyUI Text to Image Basic",
                        "workflow_execution_type": "t2i",
                        "required_base_volume_size_bytes": 85899345920,
                        "runtime_contract": {
                            "id": "comfyui-hidream-o1-dev",
                            "version": "1.0.0"
                        },
                        "provisioner_contract": {
                            "id": "luma-forge-provisioner",
                            "version": "1.0.0"
                        },
                        "required_model_assets": [
                            {
                                "id": "hidream-o1-dev-checkpoint",
                                "name": "HiDream O1 Dev",
                                "download_source": {
                                    "source_type": "huggingface",
                                    "repository_id": "Comfy-Org/HiDream-O1-Image",
                                    "file_path": "hidream_o1_image_dev_fp8_scaled.safetensors",
                                    "revision": "462165984030d82259a11f4367a4eed129e94a7b"
                                },
                                "install_comfyui_relative_path": "models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors"
                            }
                        ]
                    }
                ]
            }"#,
            &runtime_catalog,
            &provisioner_catalog,
        )
        .expect_err("missing hugging face auth flag should fail parsing");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_workflow_catalog_maps_invalid_catalog_to_validation_failed() {
        let runtime_catalog = valid_runtime_catalog();
        let provisioner_catalog = valid_provisioner_catalog();
        let invalid_catalogs = [
            ("empty workflow presets", r#"{"workflow_presets":[]}"#),
            (
                "blank preset id",
                r#"{
                    "workflow_presets": [
                        {
                            "id": " ",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "requires_hugging_face_api_key": false,
                            "runtime_contract": {
                                "id": "comfyui-hidream-o1-dev",
                                "version": "1.0.0"
                            },
                            "provisioner_contract": {
                                "id": "luma-forge-provisioner",
                                "version": "1.0.0"
                            },
                            "required_model_assets": []
                        }
                    ]
                }"#,
            ),
            (
                "zero base volume size",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-hidream-o1-dev",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 0,
                            "requires_hugging_face_api_key": false,
                            "runtime_contract": {
                                "id": "comfyui-hidream-o1-dev",
                                "version": "1.0.0"
                            },
                            "provisioner_contract": {
                                "id": "luma-forge-provisioner",
                                "version": "1.0.0"
                            },
                            "required_model_assets": []
                        }
                    ]
                }"#,
            ),
            (
                "stale runtime contract",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-hidream-o1-dev",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "requires_hugging_face_api_key": false,
                            "runtime_contract": {
                                "id": "missing-runtime-contract",
                                "version": "1.0.0"
                            },
                            "provisioner_contract": {
                                "id": "luma-forge-provisioner",
                                "version": "1.0.0"
                            },
                            "required_model_assets": []
                        }
                    ]
                }"#,
            ),
            (
                "stale provisioner contract",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-hidream-o1-dev",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "requires_hugging_face_api_key": false,
                            "runtime_contract": {
                                "id": "comfyui-hidream-o1-dev",
                                "version": "1.0.0"
                            },
                            "provisioner_contract": {
                                "id": "missing-provisioner-contract",
                                "version": "1.0.0"
                            },
                            "required_model_assets": []
                        }
                    ]
                }"#,
            ),
            (
                "unsafe model install path",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-hidream-o1-dev",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "requires_hugging_face_api_key": false,
                            "runtime_contract": {
                                "id": "comfyui-hidream-o1-dev",
                                "version": "1.0.0"
                            },
                            "provisioner_contract": {
                                "id": "luma-forge-provisioner",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [
                                {
                                    "id": "hidream-o1-dev-checkpoint",
                                    "name": "HiDream O1 Dev",
                                    "download_source": {
                                        "source_type": "huggingface",
                                        "repository_id": "Comfy-Org/HiDream-O1-Image",
                                        "file_path": "hidream_o1_image_dev_fp8_scaled.safetensors",
                                        "revision": "462165984030d82259a11f4367a4eed129e94a7b"
                                    },
                                    "install_comfyui_relative_path": "../models/checkpoints/hidream_o1_image_dev_fp8_scaled.safetensors"
                                }
                            ]
                        }
                    ]
                }"#,
            ),
        ];

        for (case, value) in invalid_catalogs {
            let err = parse_workflow_catalog(value, &runtime_catalog, &provisioner_catalog)
                .expect_err(case);
            assert_eq!(err, BundledCatalogError::ValidationFailed, "{case}");
        }
    }
}
