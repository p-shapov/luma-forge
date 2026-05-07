export type WorkspaceProvisioningProcessStatus =
  | "idle"
  | "running"
  | "failed"
  | "completed"
  | "cancelled";

export type WorkspaceProvisioningPhase =
  | "creating_persistent_storage_volume"
  | "starting_provisioning_pod"
  | "waiting_for_provisioning_worker"
  | "downloading_assets"
  | "installing_comfyui"
  | "installing_custom_nodes"
  | "validating_environment"
  | "terminating_provisioning_pod"
  | "creating_serverless_endpoint"
  | "validating_readiness";

export type WorkspaceProvisioningProgress = {
  status: WorkspaceProvisioningProcessStatus;
  phase: WorkspaceProvisioningPhase | null;
  progress_percent: number | null;
  diagnostic_message?: string;
  updated_at: string;
}
