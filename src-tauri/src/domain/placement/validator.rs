use crate::domain::{
    provider_setup::GpuCloudProviderId, validation::is_blank, workflow::WorkflowCatalog,
};

use super::PlacementPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementValidationError {
    ProviderMismatch,
    DatacenterRequired,
    GpuRequired,
    WorkflowPresetStale,
    StorageSizeBelowPresetMinimum,
}

pub fn validate_placement_plan(
    provider_id: GpuCloudProviderId,
    placement_plan: &PlacementPlan,
    workflow_catalog: &WorkflowCatalog,
) -> Result<(), PlacementValidationError> {
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        persistent_storage_volume_size_bytes,
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

    if *persistent_storage_volume_size_bytes < preset.required_base_volume_size_bytes {
        return Err(PlacementValidationError::StorageSizeBelowPresetMinimum);
    }

    Ok(())
}
