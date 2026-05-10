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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provider_inventory::{Datacenter, GpuOption},
        provider_setup::GpuCloudProviderId,
    };

    #[test]
    fn rejects_nested_provider_mismatch() {
        let inventory = ProviderInventory {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            fetched_at: "2026-05-08T00:00:00Z".to_string(),
            max_persistent_storage_volume_size_bytes: Some(1),
            datacenters: vec![Datacenter {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                id: "EU-RO-1".to_string(),
                name: "EU RO 1".to_string(),
                gpu_options: vec![GpuOption {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    id: " ".to_string(),
                    name: "NVIDIA RTX 4090".to_string(),
                    vram_bytes: 24 * 1024 * 1024 * 1024,
                    availability_score: 100,
                }],
            }],
        };

        let error = validate_provider_inventory(GpuCloudProviderId::Runpod, &inventory)
            .expect_err("blank GPU id should fail");

        assert_eq!(error, DomainValidationError);
    }
}
