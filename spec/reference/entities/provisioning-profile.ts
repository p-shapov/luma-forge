import type { EnvironmentVariables } from "../shared/environment";
import type { GpuCloudProviderId } from "./provider-setup";

export type ProvisioningComputeType = "pod";

export type ProvisioningStatusEndpoint = {
  port: number;
  protocol: "http";
  status_path: string;
}

export type ProvisionerWorkerRuntime = {
  provisioner_version: string;
  docker_image_ref: string;
  volume_mount_path: string;
  container_disk_bytes: number;
  compute_type: ProvisioningComputeType;
  status_endpoint: ProvisioningStatusEndpoint;
}

export type RunPodProvisioningProfileConfig = {
  cloud_type?: "secure" | "community";
  pod_template_id?: string;
  network_volume_mount_path: string;
  expose_http_ports: number[];
  env?: EnvironmentVariables;
}

export type ProvisioningProfileProviderConfig =
| RunPodProvisioningProfileConfig;

export type ProvisioningProfileBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  id: string;
  name: string;
  provisioner_worker_runtime: ProvisionerWorkerRuntime;
}

export type RunPodProvisioningProfile = {
  gpu_cloud_provider_id: "runpod";
  gpu_cloud_provider_config: ProvisioningProfileProviderConfig;
} & ProvisioningProfileBase;

export type ProvisioningProfile =
  RunPodProvisioningProfile;
