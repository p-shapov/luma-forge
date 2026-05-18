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
    use crate::domain::provider_inventory::{Datacenter, GpuOption};

    fn valid_gpu(id: &str) -> GpuOption {
        GpuOption {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            id: id.to_string(),
            name: format!("GPU {id}"),
            vram_bytes: 24 * 1024 * 1024 * 1024,
            availability_score: 80,
        }
    }

    fn valid_datacenter(id: &str) -> Datacenter {
        Datacenter {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            id: id.to_string(),
            name: format!("Datacenter {id}"),
            gpu_options: vec![valid_gpu("gpu-a")],
        }
    }

    fn valid_inventory() -> ProviderInventory {
        ProviderInventory {
            gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
            fetched_at: "2026-05-18T00:00:00Z".to_string(),
            max_persistent_storage_volume_size_bytes: Some(100 * 1024 * 1024 * 1024),
            datacenters: vec![valid_datacenter("dc-a")],
        }
    }

    #[test]
    fn validate_provider_inventory_accepts_valid_inventory() {
        assert_eq!(
            validate_provider_inventory(GpuCloudProviderId::Runpod, &valid_inventory()),
            Ok(())
        );
    }

    #[test]
    fn validate_provider_inventory_rejects_invalid_inventory_metadata() {
        let invalid_inventories = [
            ProviderInventory {
                fetched_at: " ".to_string(),
                ..valid_inventory()
            },
            ProviderInventory {
                max_persistent_storage_volume_size_bytes: Some(0),
                ..valid_inventory()
            },
        ];

        for inventory in invalid_inventories {
            assert_eq!(
                validate_provider_inventory(GpuCloudProviderId::Runpod, &inventory),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_provider_inventory_rejects_duplicate_or_blank_datacenters() {
        let invalid_inventories = [
            ProviderInventory {
                datacenters: vec![valid_datacenter("dc-a"), valid_datacenter("dc-a")],
                ..valid_inventory()
            },
            ProviderInventory {
                datacenters: vec![Datacenter {
                    id: " ".to_string(),
                    ..valid_datacenter("dc-a")
                }],
                ..valid_inventory()
            },
            ProviderInventory {
                datacenters: vec![Datacenter {
                    name: " ".to_string(),
                    ..valid_datacenter("dc-a")
                }],
                ..valid_inventory()
            },
        ];

        for inventory in invalid_inventories {
            assert_eq!(
                validate_provider_inventory(GpuCloudProviderId::Runpod, &inventory),
                Err(DomainValidationError)
            );
        }
    }

    #[test]
    fn validate_provider_inventory_rejects_invalid_gpu_options_per_datacenter() {
        let invalid_datacenters = [
            Datacenter {
                gpu_options: vec![valid_gpu("gpu-a"), valid_gpu("gpu-a")],
                ..valid_datacenter("dc-a")
            },
            Datacenter {
                gpu_options: vec![GpuOption {
                    id: " ".to_string(),
                    ..valid_gpu("gpu-a")
                }],
                ..valid_datacenter("dc-a")
            },
            Datacenter {
                gpu_options: vec![GpuOption {
                    name: " ".to_string(),
                    ..valid_gpu("gpu-a")
                }],
                ..valid_datacenter("dc-a")
            },
            Datacenter {
                gpu_options: vec![GpuOption {
                    vram_bytes: 0,
                    ..valid_gpu("gpu-a")
                }],
                ..valid_datacenter("dc-a")
            },
        ];

        for datacenter in invalid_datacenters {
            let inventory = ProviderInventory {
                datacenters: vec![datacenter],
                ..valid_inventory()
            };

            assert_eq!(
                validate_provider_inventory(GpuCloudProviderId::Runpod, &inventory),
                Err(DomainValidationError)
            );
        }
    }
}
