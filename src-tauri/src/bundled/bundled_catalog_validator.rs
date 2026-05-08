use std::collections::HashSet;

use crate::{
    bundled::bundled_catalog_error::BundledCatalogError,
    workspace::workspace_contracts::{EndpointProfile, ProvisioningProfile, WorkflowCatalog},
};

pub(super) fn validate_workflow_catalog(
    catalog: &WorkflowCatalog,
) -> Result<(), BundledCatalogError> {
    if is_blank(&catalog.id) || is_blank(&catalog.version) || catalog.workflow_presets.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for preset in &catalog.workflow_presets {
        if is_blank(&preset.id)
            || is_blank(&preset.version)
            || is_blank(&preset.name)
            || preset.required_base_volume_size_bytes == 0
            || !ids.insert(preset.id.as_str())
        {
            return Err(BundledCatalogError::ValidationFailed);
        }
    }

    Ok(())
}

pub(super) fn validate_provisioning_profiles(
    profiles: &[ProvisioningProfile],
) -> Result<(), BundledCatalogError> {
    if profiles.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(BundledCatalogError::ValidationFailed);
        }

        match profile {
            ProvisioningProfile::Runpod {
                name,
                version,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
                ..
            } => {
                if is_blank(name)
                    || is_blank(version)
                    || is_blank(&provisioner_worker_runtime.provisioner_version)
                    || is_blank(&provisioner_worker_runtime.volume_mount_path)
                    || provisioner_worker_runtime.container_disk_bytes == 0
                    || provisioner_worker_runtime.status_endpoint.port == 0
                    || is_blank(&provisioner_worker_runtime.status_endpoint.status_path)
                    || is_blank(&gpu_cloud_provider_config.network_volume_mount_path)
                {
                    return Err(BundledCatalogError::ValidationFailed);
                }
            }
        }
    }

    Ok(())
}

pub(super) fn validate_endpoint_profiles(
    profiles: &[EndpointProfile],
) -> Result<(), BundledCatalogError> {
    if profiles.is_empty() {
        return Err(BundledCatalogError::ValidationFailed);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(BundledCatalogError::ValidationFailed);
        }

        match profile {
            EndpointProfile::Runpod {
                name,
                version,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
                ..
            } => {
                if is_blank(name)
                    || is_blank(version)
                    || is_blank(&endpoint_worker_runtime.endpoint_worker_version)
                    || endpoint_worker_runtime.http_port == 0
                    || is_blank(&endpoint_worker_runtime.health_path)
                    || is_blank(&endpoint_worker_runtime.invoke_path)
                    || gpu_cloud_provider_config.container_disk_bytes == 0
                    || is_blank(&gpu_cloud_provider_config.volume_mount_path)
                    || gpu_cloud_provider_config.scaling.max_workers
                        < gpu_cloud_provider_config.scaling.min_workers
                {
                    return Err(BundledCatalogError::ValidationFailed);
                }
            }
        }
    }

    Ok(())
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}
