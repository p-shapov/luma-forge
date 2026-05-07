import type { GpuCloudProviderId } from "./provider-setup";

export type GpuOptionBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  id: string;
  name: string;
  vram_bytes: number;
}

export type RunPodGpuOption = {
  gpu_cloud_provider_id: "runpod";
  availability_score: number;
} & GpuOptionBase;

export type DatacenterBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  id: string;
  name: string;
}

export type RunPodDatacenter = {
  gpu_cloud_provider_id: "runpod";
  gpu_options: RunPodGpuOption[];
} & DatacenterBase;

export type ProviderInventoryBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  fetched_at: string;
}

export type RunPodProviderInventory = ProviderInventoryBase & {
  gpu_cloud_provider_id: "runpod";
  max_persistent_storage_volume_size_bytes?: number;
  datacenters: RunPodDatacenter[];
}

export type ProviderInventory =
  | RunPodProviderInventory;
