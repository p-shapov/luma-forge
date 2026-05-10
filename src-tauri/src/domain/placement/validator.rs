use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    profiles::{EndpointProfile, ProvisioningProfile},
    provider_setup::GpuCloudProviderId,
    validation::is_blank,
    workflow::WorkflowCatalog,
};

use super::PlacementPlan;

pub fn validate_placement_plan(
    provider_id: GpuCloudProviderId,
    placement_plan: &PlacementPlan,
    workflow_catalog: &WorkflowCatalog,
    provisioning_profiles: &[ProvisioningProfile],
    endpoint_profiles: &[EndpointProfile],
) -> DomainValidationResult {
    let PlacementPlan::Runpod {
        selected_datacenter_id,
        selected_gpu_id,
        persistent_storage_volume_size_bytes,
        selected_workflow_preset,
        selected_provisioning_profile,
        selected_endpoint_profile,
    } = placement_plan;

    if placement_plan.gpu_cloud_provider_id() != provider_id
        || selected_provisioning_profile.gpu_cloud_provider_id() != provider_id
        || selected_endpoint_profile.gpu_cloud_provider_id() != provider_id
        || is_blank(selected_datacenter_id)
        || is_blank(selected_gpu_id)
    {
        return Err(DomainValidationError);
    }

    let preset = workflow_catalog
        .workflow_presets
        .iter()
        .find(|preset| preset.id == selected_workflow_preset.id)
        .ok_or(DomainValidationError)?;
    if preset != selected_workflow_preset {
        return Err(DomainValidationError);
    }

    let provisioning_profile = provisioning_profiles
        .iter()
        .find(|profile| profile.id() == selected_provisioning_profile.id())
        .ok_or(DomainValidationError)?;
    if provisioning_profile != selected_provisioning_profile {
        return Err(DomainValidationError);
    }

    let endpoint_profile = endpoint_profiles
        .iter()
        .find(|profile| profile.id() == selected_endpoint_profile.id())
        .ok_or(DomainValidationError)?;
    if endpoint_profile != selected_endpoint_profile
        || endpoint_profile.workflow_execution_type()
            != selected_workflow_preset.workflow_execution_type
    {
        return Err(DomainValidationError);
    }

    if *persistent_storage_volume_size_bytes < preset.required_base_volume_size_bytes {
        return Err(DomainValidationError);
    }

    Ok(())
}
