export type WorkspaceProvisioningStatus =
  | "idle"
  | "running"
  | "cancelling"
  | "completed"
  | "failed";

export type WorkspaceProvisioningFailureCode =
  | "provider_resource_failed"
  | "provider_resource_terminated"
  | "provider_resource_unknown"
  | "provider_resource_missing"
  | "provider_operation_indeterminate"
  | "provisioner_worker_token_missing"
  | "provisioner_worker_token_invalid"
  | "provisioner_worker_unauthorized"
  | "provisioner_worker_response_invalid"
  | "provisioner_worker_failed"
  | "readiness_validation_failed"
  | "cancellation_cleanup_failed"
  | "legacy_failure";

export type WorkspaceProvisioningFailureSource =
  | "native"
  | "provider"
  | "provider_resource"
  | "provisioner_worker";

export type WorkspaceProvisioningRecoveryAction =
  | "retry"
  | "recover_provider_setup"
  | "reselect_placement"
  | "inspect_workspace_provisioning"
  | "cleanup_workspace_resources";

export type WorkspaceProvisioningFailure = {
  code: WorkspaceProvisioningFailureCode;
  phase: WorkspaceProvisioningPhase;
  source: WorkspaceProvisioningFailureSource;
  retryable: boolean;
  recovery_action: WorkspaceProvisioningRecoveryAction;
  diagnostic: string | null;
};

export type WorkspaceProvisioningPhase =
  | "not_started"
  | "creating_volume"
  | "starting_provisioning_pod"
  | "preparing_environment"
  | "creating_endpoint_template"
  | "creating_endpoint"
  | "validating_readiness"
  | "cleaning_up"
  | "completed"
  | "failed";

export type WorkspaceProvisioningProgress = {
  status: WorkspaceProvisioningStatus;
  phase: WorkspaceProvisioningPhase;
  percent: number | null;
  failure: WorkspaceProvisioningFailure | null;
};
