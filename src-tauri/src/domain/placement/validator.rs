use crate::domain::{
    provider_setup::GpuCloudProviderId,
    provisioner::{validator as provisioner_validator, ProvisionerCatalog},
    runtime::{validator as runtime_validator, RuntimeCatalog},
    validation::is_blank,
    workflow::WorkflowCatalog,
};

use super::PlacementPlan;
use super::{RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS, RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementValidationError {
    ProviderMismatch,
    DatacenterRequired,
    GpuRequired,
    WorkflowPresetStale,
    StorageSizeBelowPresetMinimum,
    EndpointKeepAliveOutOfRange,
}

pub fn validate_placement_plan(
    provider_id: GpuCloudProviderId,
    placement_plan: &PlacementPlan,
    workflow_catalog: &WorkflowCatalog,
    runtime_catalog: &RuntimeCatalog,
    provisioner_catalog: &ProvisionerCatalog,
) -> Result<(), PlacementValidationError> {
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        persistent_storage_volume_size_bytes,
        endpoint_keep_alive_seconds,
        selected_workflow_preset,
    } = placement_plan;

    if placement_plan.gpu_cloud_provider_id() != provider_id {
        return Err(PlacementValidationError::ProviderMismatch);
    }
    if is_blank(selected_datacenter_id) {
        return Err(PlacementValidationError::DatacenterRequired);
    }
    if is_blank(selected_gpu_id) {
        return Err(PlacementValidationError::GpuRequired);
    }

    let preset = workflow_catalog
        .workflow_presets
        .iter()
        .find(|preset| preset.id == selected_workflow_preset.id)
        .ok_or(PlacementValidationError::WorkflowPresetStale)?;
    if preset != selected_workflow_preset {
        return Err(PlacementValidationError::WorkflowPresetStale);
    }
    runtime_validator::validate_runtime_contract_reference(
        &selected_workflow_preset.runtime_contract.id,
        &selected_workflow_preset.runtime_contract.version,
        runtime_catalog,
    )
    .map_err(|_| PlacementValidationError::WorkflowPresetStale)?;
    provisioner_validator::validate_provisioner_contract_reference(
        &selected_workflow_preset.provisioner_contract.id,
        &selected_workflow_preset.provisioner_contract.version,
        provisioner_catalog,
    )
    .map_err(|_| PlacementValidationError::WorkflowPresetStale)?;

    if *persistent_storage_volume_size_bytes < preset.required_base_volume_size_bytes {
        return Err(PlacementValidationError::StorageSizeBelowPresetMinimum);
    }
    if !(RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS..=RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS)
        .contains(endpoint_keep_alive_seconds)
    {
        return Err(PlacementValidationError::EndpointKeepAliveOutOfRange);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        placement::{
            RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS, RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS,
            RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS,
        },
        provisioner::{ProvisionerCatalog, ProvisionerContract, ProvisionerContractRevision},
        runtime::{RuntimeContract, RuntimeContractRevision},
        workflow::{
            ProvisionerContractReference, RuntimeContractReference, WorkflowExecutionType,
            WorkflowPreset,
        },
    };

    const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const REQUIRED_VOLUME_SIZE: u64 = 80 * 1024 * 1024 * 1024;

    fn runtime_catalog() -> RuntimeCatalog {
        RuntimeCatalog {
            contracts: vec![RuntimeContract {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                revisions: vec![RuntimeContractRevision {
                    version: "1.0.0".to_string(),
                    endpoint_image_ref: format!("ghcr.io/luma-forge/endpoint@sha256:{DIGEST_B}"),
                }],
            }],
        }
    }

    fn provisioner_catalog() -> ProvisionerCatalog {
        ProvisionerCatalog {
            contracts: vec![ProvisionerContract {
                id: "luma-forge-provisioner".to_string(),
                revisions: vec![ProvisionerContractRevision {
                    version: "1.0.0".to_string(),
                    provisioner_worker_image_ref: format!(
                        "ghcr.io/luma-forge/provisioner@sha256:{DIGEST_C}"
                    ),
                    volume_mount_path: "/workspace".to_string(),
                }],
            }],
        }
    }

    fn workflow_preset() -> WorkflowPreset {
        WorkflowPreset {
            id: "comfyui-hidream-o1-dev".to_string(),
            version: "1.0.0".to_string(),
            name: "ComfyUI Text to Image".to_string(),
            workflow_execution_type: WorkflowExecutionType::T2i,
            required_base_volume_size_bytes: REQUIRED_VOLUME_SIZE,
            runtime_contract: RuntimeContractReference {
                id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                version: "1.0.0".to_string(),
            },
            provisioner_contract: ProvisionerContractReference {
                id: "luma-forge-provisioner".to_string(),
                version: "1.0.0".to_string(),
            },
            required_model_assets: vec![],
        }
    }

    fn workflow_catalog() -> WorkflowCatalog {
        WorkflowCatalog {
            workflow_presets: vec![workflow_preset()],
        }
    }

    fn placement_plan() -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: "EU-RO-1".to_string(),
            selected_gpu_id: "NVIDIA A40".to_string(),
            persistent_storage_volume_size_bytes: REQUIRED_VOLUME_SIZE,
            endpoint_keep_alive_seconds: RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            selected_workflow_preset: workflow_preset(),
        }
    }

    fn plan_with(
        datacenter_id: &str,
        gpu_id: &str,
        volume_size: u64,
        keep_alive_seconds: u32,
        selected_workflow_preset: WorkflowPreset,
    ) -> PlacementPlan {
        PlacementPlan::Runpod {
            selected_datacenter_id: datacenter_id.to_string(),
            selected_gpu_id: gpu_id.to_string(),
            persistent_storage_volume_size_bytes: volume_size,
            endpoint_keep_alive_seconds: keep_alive_seconds,
            selected_workflow_preset,
        }
    }

    #[test]
    fn validate_placement_plan_accepts_valid_plan() {
        assert_eq!(
            validate_placement_plan(
                GpuCloudProviderId::Runpod,
                &placement_plan(),
                &workflow_catalog(),
                &runtime_catalog(),
                &provisioner_catalog(),
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_placement_plan_accepts_keep_alive_boundaries() {
        for keep_alive_seconds in [
            RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS,
            RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS,
        ] {
            let plan = plan_with(
                "EU-RO-1",
                "NVIDIA A40",
                REQUIRED_VOLUME_SIZE,
                keep_alive_seconds,
                workflow_preset(),
            );

            assert_eq!(
                validate_placement_plan(
                    GpuCloudProviderId::Runpod,
                    &plan,
                    &workflow_catalog(),
                    &runtime_catalog(),
                    &provisioner_catalog(),
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn validate_placement_plan_rejects_missing_selection_fields() {
        let invalid_plans = [
            (
                plan_with(
                    " ",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE,
                    RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
                    workflow_preset(),
                ),
                PlacementValidationError::DatacenterRequired,
            ),
            (
                plan_with(
                    "EU-RO-1",
                    " ",
                    REQUIRED_VOLUME_SIZE,
                    RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
                    workflow_preset(),
                ),
                PlacementValidationError::GpuRequired,
            ),
        ];

        for (plan, expected_error) in invalid_plans {
            assert_eq!(
                validate_placement_plan(
                    GpuCloudProviderId::Runpod,
                    &plan,
                    &workflow_catalog(),
                    &runtime_catalog(),
                    &provisioner_catalog(),
                ),
                Err(expected_error)
            );
        }
    }

    #[test]
    fn validate_placement_plan_rejects_stale_or_changed_workflow_preset() {
        let missing_preset = plan_with(
            "EU-RO-1",
            "NVIDIA A40",
            REQUIRED_VOLUME_SIZE,
            RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            WorkflowPreset {
                id: "missing-preset".to_string(),
                ..workflow_preset()
            },
        );
        assert_eq!(
            validate_placement_plan(
                GpuCloudProviderId::Runpod,
                &missing_preset,
                &workflow_catalog(),
                &runtime_catalog(),
                &provisioner_catalog(),
            ),
            Err(PlacementValidationError::WorkflowPresetStale)
        );

        let changed_preset = plan_with(
            "EU-RO-1",
            "NVIDIA A40",
            REQUIRED_VOLUME_SIZE,
            RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            WorkflowPreset {
                name: "Changed".to_string(),
                ..workflow_preset()
            },
        );
        assert_eq!(
            validate_placement_plan(
                GpuCloudProviderId::Runpod,
                &changed_preset,
                &workflow_catalog(),
                &runtime_catalog(),
                &provisioner_catalog(),
            ),
            Err(PlacementValidationError::WorkflowPresetStale)
        );
    }

    #[test]
    fn validate_placement_plan_rejects_stale_runtime_contract_reference() {
        let stale_runtime = plan_with(
            "EU-RO-1",
            "NVIDIA A40",
            REQUIRED_VOLUME_SIZE,
            RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            WorkflowPreset {
                runtime_contract: RuntimeContractReference {
                    id: "comfyui-hidream-o1-dev-python312-cu121".to_string(),
                    version: "2.0.0".to_string(),
                },
                ..workflow_preset()
            },
        );
        let catalog = WorkflowCatalog {
            workflow_presets: vec![stale_runtime.selected_workflow_preset().clone()],
        };

        assert_eq!(
            validate_placement_plan(
                GpuCloudProviderId::Runpod,
                &stale_runtime,
                &catalog,
                &runtime_catalog(),
                &provisioner_catalog(),
            ),
            Err(PlacementValidationError::WorkflowPresetStale)
        );
    }

    #[test]
    fn validate_placement_plan_rejects_stale_provisioner_contract_reference() {
        let stale_provisioner = plan_with(
            "EU-RO-1",
            "NVIDIA A40",
            REQUIRED_VOLUME_SIZE,
            RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
            WorkflowPreset {
                provisioner_contract: ProvisionerContractReference {
                    id: "luma-forge-provisioner".to_string(),
                    version: "2.0.0".to_string(),
                },
                ..workflow_preset()
            },
        );
        let catalog = WorkflowCatalog {
            workflow_presets: vec![stale_provisioner.selected_workflow_preset().clone()],
        };

        assert_eq!(
            validate_placement_plan(
                GpuCloudProviderId::Runpod,
                &stale_provisioner,
                &catalog,
                &runtime_catalog(),
                &provisioner_catalog(),
            ),
            Err(PlacementValidationError::WorkflowPresetStale)
        );
    }

    #[test]
    fn validate_placement_plan_rejects_volume_or_keep_alive_out_of_range() {
        let invalid_plans = [
            (
                plan_with(
                    "EU-RO-1",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE - 1,
                    RUNPOD_ENDPOINT_KEEP_ALIVE_DEFAULT_SECONDS,
                    workflow_preset(),
                ),
                PlacementValidationError::StorageSizeBelowPresetMinimum,
            ),
            (
                plan_with(
                    "EU-RO-1",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE,
                    RUNPOD_ENDPOINT_KEEP_ALIVE_MIN_SECONDS - 1,
                    workflow_preset(),
                ),
                PlacementValidationError::EndpointKeepAliveOutOfRange,
            ),
            (
                plan_with(
                    "EU-RO-1",
                    "NVIDIA A40",
                    REQUIRED_VOLUME_SIZE,
                    RUNPOD_ENDPOINT_KEEP_ALIVE_MAX_SECONDS + 1,
                    workflow_preset(),
                ),
                PlacementValidationError::EndpointKeepAliveOutOfRange,
            ),
        ];

        for (plan, expected_error) in invalid_plans {
            assert_eq!(
                validate_placement_plan(
                    GpuCloudProviderId::Runpod,
                    &plan,
                    &workflow_catalog(),
                    &runtime_catalog(),
                    &provisioner_catalog(),
                ),
                Err(expected_error)
            );
        }
    }
}
