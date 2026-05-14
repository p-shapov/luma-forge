import type { GpuCloudProviderId } from "./provider-setup";

export type ProviderResourceStatus =
  | "creating"
  | "running"
  | "ready"
  | "terminated"
  | "failed"
  | "unknown";

type ProviderResourceSnapshotBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  provider_resource_id: string;
  datacenter_id: string;
  provider_resource_status: ProviderResourceStatus;
}

export type PersistentStorageVolumeSnapshot = ProviderResourceSnapshotBase & {
  kind: "persistent_storage_volume";
  provisioned_size_bytes: number;
  mount_path: string;
}

export type ProvisioningPodSnapshot = ProviderResourceSnapshotBase & {
  kind: "provisioning_pod";
  selected_gpu_id: string;
  provisioner_status_url: string;
}

export type ServerlessEndpointSnapshot = ProviderResourceSnapshotBase & {
  kind: "serverless_endpoint";
  selected_gpu_id: string;
  endpoint_invoke_url: string;
}
