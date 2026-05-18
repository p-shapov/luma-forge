use std::collections::HashSet;

use crate::domain::{
    error::{DomainValidationError, DomainValidationResult},
    provider_setup::GpuCloudProviderId,
    validation::is_blank,
};

use super::ProviderInventory;

pub fn validate_provider_inventory(
    provider_id: GpuCloudProviderId,
    inventory: &ProviderInventory,
) -> DomainValidationResult {
    if inventory.gpu_cloud_provider_id != provider_id
        || is_blank(&inventory.fetched_at)
        || inventory
            .max_persistent_storage_volume_size_bytes
            .is_some_and(|size| size == 0)
    {
        return Err(DomainValidationError);
    }

    let mut datacenter_ids = HashSet::new();
    for datacenter in &inventory.datacenters {
        if datacenter.gpu_cloud_provider_id != provider_id
            || is_blank(&datacenter.id)
            || is_blank(&datacenter.name)
            || !datacenter_ids.insert(datacenter.id.as_str())
        {
            return Err(DomainValidationError);
        }

        let mut gpu_ids = HashSet::new();
        for gpu_option in &datacenter.gpu_options {
            if gpu_option.gpu_cloud_provider_id != provider_id
                || is_blank(&gpu_option.id)
                || is_blank(&gpu_option.name)
                || gpu_option.vram_bytes == 0
                || !gpu_ids.insert(gpu_option.id.as_str())
            {
                return Err(DomainValidationError);
            }
        }
    }

    Ok(())
}
