import type { DockerImage } from "../shared/docker-image";
import type { EnvironmentVariables } from "../shared/environment";
import type { GpuCloudProviderId } from "./provider-setup";
import type { WorkflowExecutionType } from "./workflow-preset";

export type EndpointWorkerRuntime = {
  endpoint_worker_version: string;
  docker_image: DockerImage;
  http_port: number;
  health_path: string;
  invoke_path: string;
}

export type RunPodServerlessScalingConfig = {
  min_workers: number;
  max_workers: number;
  idle_timeout_seconds: number;
  scaler_type?: "queue_delay" | "request_count";
  scaler_value?: number;
}

export type RunPodEndpointProfileConfig = {
  endpoint_template_id?: string;
  container_disk_bytes: number;
  volume_mount_path: string;
  env?: EnvironmentVariables;
  scaling: RunPodServerlessScalingConfig;
}

export type EndpointProfileBase = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  id: string;
  name: string;
  workflow_execution_type: WorkflowExecutionType;
  endpoint_worker_runtime: EndpointWorkerRuntime;
}

export type RunPodEndpointProfile = {
  gpu_cloud_provider_id: "runpod";
  gpu_cloud_provider_config: RunPodEndpointProfileConfig;
} & EndpointProfileBase;

export type EndpointProfile =
 | RunPodEndpointProfile;
