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
  | "provider_orphaned_resources"
  | "provider_setup_incomplete"
  | "provider_api_key_unauthorized"
  | "provider_api_unavailable"
  | "provider_rate_limited"
  | "provider_request_rejected"
  | "provider_response_invalid"
  | "provider_operation_conflict"
  | "provider_operation_indeterminate"
  | "provisioner_worker_token_missing"
  | "provisioner_worker_token_invalid"
  | "provisioner_worker_unauthorized"
  | "provisioner_worker_unavailable"
  | "provisioner_worker_conflict"
  | "provisioner_worker_response_invalid"
  | "provisioner_worker_failed"
  | "provisioner_worker_git_checkout_failed"
  | "provisioner_worker_dependency_install_failed"
  | "provisioner_worker_asset_download_failed"
  | "provisioner_worker_asset_auth_required"
  | "provisioner_worker_path_validation_failed"
  | "provisioner_worker_step_timeout"
  | "provisioner_worker_unexpected_error"
  | "secure_keyring_unavailable"
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
  recovery_action: WorkspaceProvisioningRecoveryAction;
};

export type WorkspaceProvisioningPhase =
  | "not_started"
  | "creating_volume"
  | "starting_provisioning_pod"
  | "preparing_environment"
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
