export type GpuCloudProviderId = 
  | "runpod";

export type GpuCloudProviderSetup = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  provider_user_id: string;
  provider_api_key_fingerprint: string;
}
