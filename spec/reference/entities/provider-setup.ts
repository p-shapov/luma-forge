export type GpuCloudProviderId = 
  | "runpod";

export type GpuCloudProviderSetup = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  provider_user_email: string;
  provider_api_key_fingerprint: string;
}
