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
    fn parse_workflow_catalog_accepts_valid_catalog_without_custom_nodes() {
        let runtime_catalog = valid_runtime_catalog();
        let catalog = parse_workflow_catalog(valid_workflow_catalog_json(), &runtime_catalog)
            .expect("valid workflow catalog should parse");

        let [preset] = catalog.workflow_presets.as_slice() else {
            panic!("expected one workflow preset");
        };
        assert_eq!(preset.id, "comfyui-t2i-basic");
        assert_eq!(preset.required_model_assets.len(), 1);
    }

    #[test]
    fn parse_workflow_catalog_maps_invalid_json_to_parse_failed() {
        let runtime_catalog = valid_runtime_catalog();
        let err = parse_workflow_catalog("{ invalid json", &runtime_catalog)
            .expect_err("invalid JSON should fail before validation");

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
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 0,
                            "runtime_contract": {
                                "id": "comfyui-python312-cu121",
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
                            "id": "comfyui-t2i-basic",
                            "version": "1.0.0",
                            "name": "ComfyUI Text to Image Basic",
                            "workflow_execution_type": "t2i",
                            "required_base_volume_size_bytes": 85899345920,
                            "runtime_contract": {
                                "id": "missing-runtime-contract",
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
