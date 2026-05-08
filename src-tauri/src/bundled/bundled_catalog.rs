use std::collections::HashSet;

use crate::{
    bundled::bundled_contracts::{EndpointProfile, ProvisioningProfile},
    domain::workflow::WorkflowCatalog,
    workspace::workspace_setup::WorkspaceSetupError,
};

const WORKFLOW_CATALOG_JSON: &str =
    include_str!("../../../resources/catalog/workflow-catalog.json");
const PROVISIONING_PROFILES_JSON: &str =
    include_str!("../../../resources/catalog/provisioning-profiles.json");
const ENDPOINT_PROFILES_JSON: &str =
    include_str!("../../../resources/catalog/endpoint-profiles.json");

pub trait CatalogReader: Send + Sync {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError>;
    fn provisioning_profiles(&self) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError>;
    fn endpoint_profiles(&self) -> Result<Vec<EndpointProfile>, WorkspaceSetupError>;
}

#[derive(Debug, Clone, Default)]
pub struct BundledCatalogReader;

impl CatalogReader for BundledCatalogReader {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        parse_workflow_catalog(WORKFLOW_CATALOG_JSON)
    }

    fn provisioning_profiles(&self) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError> {
        parse_provisioning_profiles(PROVISIONING_PROFILES_JSON)
    }

    fn endpoint_profiles(&self) -> Result<Vec<EndpointProfile>, WorkspaceSetupError> {
        parse_endpoint_profiles(ENDPOINT_PROFILES_JSON)
    }
}

pub(super) fn parse_workflow_catalog(value: &str) -> Result<WorkflowCatalog, WorkspaceSetupError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)?;
    validate_workflow_catalog(&catalog)?;
    Ok(catalog)
}

pub(super) fn parse_provisioning_profiles(
    value: &str,
) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError> {
    let profiles: Vec<ProvisioningProfile> =
        serde_json::from_str(value).map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)?;
    validate_provisioning_profiles(&profiles)?;
    Ok(profiles)
}

fn parse_endpoint_profiles(value: &str) -> Result<Vec<EndpointProfile>, WorkspaceSetupError> {
    let profiles: Vec<EndpointProfile> =
        serde_json::from_str(value).map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)?;
    validate_endpoint_profiles(&profiles)?;
    Ok(profiles)
}

fn validate_workflow_catalog(catalog: &WorkflowCatalog) -> Result<(), WorkspaceSetupError> {
    if is_blank(&catalog.id) || is_blank(&catalog.version) || catalog.workflow_presets.is_empty() {
        return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
    }

    let mut ids = HashSet::new();
    for preset in &catalog.workflow_presets {
        if is_blank(&preset.id)
            || is_blank(&preset.version)
            || is_blank(&preset.name)
            || preset.required_base_volume_size_bytes == 0
            || !ids.insert(preset.id.as_str())
        {
            return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
        }
    }

    Ok(())
}

fn validate_provisioning_profiles(
    profiles: &[ProvisioningProfile],
) -> Result<(), WorkspaceSetupError> {
    if profiles.is_empty() {
        return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
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
                    return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
                }
            }
        }
    }

    Ok(())
}

fn validate_endpoint_profiles(profiles: &[EndpointProfile]) -> Result<(), WorkspaceSetupError> {
    if profiles.is_empty() {
        return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
    }

    let mut ids = HashSet::new();
    for profile in profiles {
        if is_blank(profile.id()) || !ids.insert(profile.id()) {
            return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
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
                    return Err(WorkspaceSetupError::WorkflowCatalogUnavailable);
                }
            }
        }
    }

    Ok(())
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

#[cfg(test)]
#[path = "bundled_catalog_tests.rs"]
mod bundled_catalog_tests;
