use crate::{
    bundled_catalog::error::BundledCatalogError,
    domain::{
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

pub(super) fn parse_workflow_catalog(
    value: &str,
    runtime_catalog: &RuntimeCatalog,
) -> Result<WorkflowCatalog, BundledCatalogError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_workflow_catalog(&catalog, runtime_catalog)
        .map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn valid_runtime_catalog_json() -> String {
        format!(
            r#"{{
                "contracts": [
                    {{
                        "id": "comfyui-python312-cu121",
                        "revisions": [
                            {{
                                "version": "1.0.0",
                                "provisioner_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_A}",
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

    fn valid_workflow_catalog_json() -> &'static str {
        r#"{
            "workflow_presets": [
                {
                    "id": "comfyui-t2i-basic",
                    "version": "1.0.0",
                    "name": "ComfyUI Text to Image Basic",
                    "workflow_execution_type": "t2i",
                    "required_base_volume_size_bytes": 85899345920,
                    "runtime_contract": {
                        "id": "comfyui-python312-cu121",
                        "version": "1.0.0"
                    },
                    "required_model_assets": [
                        {
                            "id": "sdxl-base-1-0",
                            "name": "SDXL Base 1.0",
                            "model_asset_kind": "checkpoint",
                            "download_source": {
                                "source_type": "huggingface",
                                "repository_id": "stabilityai/stable-diffusion-xl-base-1.0",
                                "file_path": "sd_xl_base_1.0.safetensors",
                                "revision": "462165984030d82259a11f4367a4eed129e94a7b"
                            },
                            "install": {
                                "comfyui_relative_path": "models/checkpoints/sd_xl_base_1.0.safetensors"
                            }
                        }
                    ],
                    "required_custom_nodes": [
                        {
                            "id": "example-custom-node",
                            "name": "Example Custom Node",
                            "git_source": {
                                "source_type": "git",
                                "repository_url": "https://github.com/example/custom-node.git",
                                "revision": "0123456789abcdef0123456789abcdef01234567"
                            },
                            "install": {
                                "comfyui_custom_nodes_relative_path": "custom_nodes/example-custom-node",
                                "python_requirements_path": "custom_nodes/example-custom-node/requirements.txt"
                            }
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
        assert_eq!(contract.id, "comfyui-python312-cu121");

        let [revision] = contract.revisions.as_slice() else {
            panic!("expected one runtime revision");
        };
        assert_eq!(revision.version, "1.0.0");
        assert!(revision
            .provisioner_image_ref
            .ends_with(&format!("@sha256:{DIGEST_A}")));
    }

    #[test]
    fn parse_runtime_catalog_maps_invalid_json_to_parse_failed() {
        let err = parse_runtime_catalog("{ invalid json")
            .expect_err("invalid JSON should fail before validation");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_runtime_catalog_maps_schema_mismatch_to_parse_failed() {
        let err = parse_runtime_catalog(r#"{"contracts":"not-an-array"}"#)
            .expect_err("schema mismatch should fail during deserialization");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_runtime_catalog_maps_invalid_catalog_to_validation_failed() {
        let invalid_catalogs = [
            ("empty contracts", r#"{"contracts":[]}"#.to_owned()),
            (
                "invalid contract id",
                format!(
                    r#"{{
                        "contracts": [
                            {{
                                "id": "ComfyUI",
                                "revisions": [
                                    {{
                                        "version": "1.0.0",
                                        "provisioner_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_A}",
                                        "endpoint_image_ref": "ghcr.io/example/endpoint@sha256:{DIGEST_B}"
                                    }}
                                ]
                            }}
                        ]
                    }}"#
                ),
            ),
            (
                "duplicate contract id",
                format!(
                    r#"{{
                        "contracts": [
                            {{
                                "id": "comfyui-python312-cu121",
                                "revisions": [
                                    {{
                                        "version": "1.0.0",
                                        "provisioner_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_A}",
                                        "endpoint_image_ref": "ghcr.io/example/endpoint@sha256:{DIGEST_B}"
                                    }}
                                ]
                            }},
                            {{
                                "id": "comfyui-python312-cu121",
                                "revisions": [
                                    {{
                                        "version": "1.0.1",
                                        "provisioner_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_A}",
                                        "endpoint_image_ref": "ghcr.io/example/endpoint@sha256:{DIGEST_B}"
                                    }}
                                ]
                            }}
                        ]
                    }}"#
                ),
            ),
            (
                "empty revisions",
                r#"{"contracts":[{"id":"comfyui-python312-cu121","revisions":[]}]}"#.to_owned(),
            ),
            (
                "invalid semver",
                format!(
                    r#"{{
                        "contracts": [
                            {{
                                "id": "comfyui-python312-cu121",
                                "revisions": [
                                    {{
                                        "version": "01.0.0",
                                        "provisioner_image_ref": "ghcr.io/example/provisioner@sha256:{DIGEST_A}",
                                        "endpoint_image_ref": "ghcr.io/example/endpoint@sha256:{DIGEST_B}"
                                    }}
                                ]
                            }}
                        ]
                    }}"#
                ),
            ),
            (
                "mutable image reference",
                r#"{
                    "contracts": [
                        {
                            "id": "comfyui-python312-cu121",
                            "revisions": [
                                {
                                    "version": "1.0.0",
                                    "provisioner_image_ref": "ghcr.io/example/provisioner:latest",
                                    "endpoint_image_ref": "ghcr.io/example/endpoint:latest"
                                }
                            ]
                        }
                    ]
                }"#
                .to_owned(),
            ),
        ];

        for (case, value) in invalid_catalogs {
            let err = parse_runtime_catalog(&value).expect_err(case);
            assert_eq!(err, BundledCatalogError::ValidationFailed, "{case}");
        }
    }

    #[test]
    fn parse_workflow_catalog_accepts_valid_catalog() {
        let runtime_catalog = valid_runtime_catalog();

        let catalog = parse_workflow_catalog(valid_workflow_catalog_json(), &runtime_catalog)
            .expect("valid workflow catalog should parse");

        let [preset] = catalog.workflow_presets.as_slice() else {
            panic!("expected one workflow preset");
        };
        assert_eq!(preset.id, "comfyui-t2i-basic");
        assert_eq!(preset.runtime_contract.id, "comfyui-python312-cu121");
    }

    #[test]
    fn parse_workflow_catalog_maps_invalid_json_to_parse_failed() {
        let runtime_catalog = valid_runtime_catalog();

        let err = parse_workflow_catalog("{ invalid json", &runtime_catalog)
            .expect_err("invalid JSON should fail before validation");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_workflow_catalog_maps_schema_mismatch_to_parse_failed() {
        let runtime_catalog = valid_runtime_catalog();

        let err = parse_workflow_catalog(
            r#"{"workflow_presets":[{"workflow_execution_type":"unknown"}]}"#,
            &runtime_catalog,
        )
        .expect_err("unknown enum variant should fail during deserialization");

        assert_eq!(err, BundledCatalogError::ParseFailed);
    }

    #[test]
    fn parse_workflow_catalog_maps_invalid_catalog_to_validation_failed() {
        let runtime_catalog = valid_runtime_catalog();
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
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [],
                            "required_custom_nodes": []
                        }
                    ]
                }"#,
            ),
            (
                "zero base volume size",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 0,
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [],
                            "required_custom_nodes": []
                        }
                    ]
                }"#,
            ),
            (
                "stale runtime contract",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "runtime_contract": {
                                "id": "missing-runtime-contract",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [],
                            "required_custom_nodes": []
                        }
                    ]
                }"#,
            ),
            (
                "unsafe model install path",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [
                                {
                                    "id": "sdxl-base-1-0",
                                    "name": "SDXL Base 1.0",
                                    "model_asset_kind": "checkpoint",
                                    "download_source": {
                                        "source_type": "huggingface",
                                        "repository_id": "stabilityai/stable-diffusion-xl-base-1.0",
                                        "file_path": "sd_xl_base_1.0.safetensors",
                                        "revision": "462165984030d82259a11f4367a4eed129e94a7b"
                                    },
                                    "install": {
                                        "comfyui_relative_path": "../models/checkpoints/sd_xl_base_1.0.safetensors"
                                    }
                                }
                            ],
                            "required_custom_nodes": []
                        }
                    ]
                }"#,
            ),
            (
                "mutable custom node revision",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [],
                            "required_custom_nodes": [
                                {
                                    "id": "example-custom-node",
                                    "name": "Example Custom Node",
                                    "git_source": {
                                        "source_type": "git",
                                        "repository_url": "https://github.com/example/custom-node.git",
                                        "revision": "main"
                                    },
                                    "install": {
                                        "comfyui_custom_nodes_relative_path": "custom_nodes/example-custom-node",
                                        "python_requirements_path": null
                                    }
                                }
                            ]
                        }
                    ]
                }"#,
            ),
            (
                "custom node outside custom_nodes",
                r#"{
                    "workflow_presets": [
                        {
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
                                "version": "1.0.0"
                            },
                            "required_model_assets": [],
                            "required_custom_nodes": [
                                {
                                    "id": "example-custom-node",
                                    "name": "Example Custom Node",
                                    "git_source": {
                                        "source_type": "git",
                                        "repository_url": "https://github.com/example/custom-node.git",
                                        "revision": "0123456789abcdef0123456789abcdef01234567"
                                    },
                                    "install": {
                                        "comfyui_custom_nodes_relative_path": "extensions/example-custom-node",
                                        "python_requirements_path": null
                                    }
                                }
                            ]
                        }
                    ]
                }"#,
            ),
        ];

        for (case, value) in invalid_catalogs {
            let err = parse_workflow_catalog(value, &runtime_catalog).expect_err(case);
            assert_eq!(err, BundledCatalogError::ValidationFailed, "{case}");
        }
    }
}
