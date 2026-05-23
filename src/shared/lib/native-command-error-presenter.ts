import type {
  NativeCommandError,
  NativeCommandErrorCode,
} from "@/generated/commands";

interface NativeCommandErrorCopy {
  title: string;
}

export interface NativeCommandErrorPresentation {
  title: string;
  description: string;
  recoveryHint: string | null;
  retryable: boolean;
  details: Array<{
    label: string;
    value: string;
  }>;
}

const ERROR_COPY = {
  provider_setup_incomplete: { title: "Provider setup incomplete" },
  provider_setup_not_found: { title: "Provider setup not found" },
  provider_setup_already_exists: { title: "Provider setup already exists" },
  provider_api_key_required: { title: "Provider API key required" },
  provider_api_key_unauthorized: { title: "Provider API key unauthorized" },
  stored_provider_api_key_invalid: { title: "Stored provider API key invalid" },
  provider_api_unavailable: { title: "Provider API unavailable" },
  provider_rate_limited: { title: "Provider rate limited" },
  provider_request_rejected: { title: "Provider request rejected" },
  provider_response_invalid: { title: "Provider response invalid" },
  provider_inventory_invalid: { title: "Provider inventory invalid" },
  provider_identity_response_invalid: { title: "Provider identity response invalid" },
  secure_keyring_unavailable: { title: "Secure keyring unavailable" },
  provider_setup_recovery_required: { title: "Provider setup recovery required" },
  workflow_catalog_unavailable: { title: "Workflow catalog unavailable" },
  workspace_catalog_unavailable: { title: "Workspace catalog unavailable" },
  workspace_catalog_storage_unavailable: { title: "Workspace catalog storage unavailable" },
  workspace_catalog_migration_failed: { title: "Workspace catalog migration failed" },
  workspace_catalog_query_failed: { title: "Workspace catalog query failed" },
  workspace_catalog_corrupt: { title: "Workspace catalog corrupt" },
  workspace_catalog_schema_mismatch: { title: "Workspace catalog schema mismatch" },
  placement_provider_mismatch: { title: "Placement provider mismatch" },
  placement_datacenter_required: { title: "Datacenter required" },
  placement_gpu_required: { title: "GPU required" },
  workflow_preset_stale: { title: "Workflow preset stale" },
  storage_size_below_preset_minimum: { title: "Storage size too small" },
  endpoint_keep_alive_out_of_range: { title: "Endpoint keep-alive out of range" },
  workspace_already_exists: { title: "Workspace already exists" },
  workspace_not_found: { title: "Workspace not found" },
  invalid_workspace_lifecycle: { title: "Invalid workspace lifecycle" },
  invalid_workspace_id: { title: "Invalid workspace ID" },
  workspace_name_required: { title: "Workspace name required" },
  invalid_workspace_metadata: { title: "Invalid workspace metadata" },
  provider_resource_not_found: { title: "Provider resource not found" },
  provider_orphaned_resources: { title: "Provider orphaned resources" },
  provider_operation_conflict: { title: "Provider operation conflict" },
  provider_operation_indeterminate: { title: "Provider operation indeterminate" },
  cleanup_failed: { title: "Cleanup failed" },
  provisioner_worker_token_invalid: { title: "Provisioner worker token invalid" },
  provisioner_worker_unauthorized: { title: "Provisioner worker unauthorized" },
  provisioner_worker_unavailable: { title: "Provisioner worker unavailable" },
  provisioner_worker_conflict: { title: "Provisioner worker conflict" },
  provisioner_worker_response_invalid: { title: "Provisioner worker response invalid" },
  provisioner_worker_failed: { title: "Provisioner worker failed" },
} satisfies Record<NativeCommandErrorCode, NativeCommandErrorCopy>;

export function isNativeCommandError(value: unknown): value is NativeCommandError {
  return typeof value === "object"
    && value !== null
    && "code" in value
    && "message" in value
    && "retryable" in value;
}

export function presentNativeCommandError(
  error: NativeCommandError,
): NativeCommandErrorPresentation {
  return {
    title: ERROR_COPY[error.code].title,
    description: error.message,
    recoveryHint: recoveryHint(error.recovery_action),
    retryable: error.retryable,
    details: [
      { label: "Code", value: error.code },
      error.field === null ? null : { label: "Field", value: error.field },
    ].filter((detail): detail is { label: string; value: string } => detail !== null),
  };
}

function recoveryHint(recoveryAction: string | null): string | null {
  switch (recoveryAction) {
    case null:
      return null;
    case "setup_provider":
      return "Complete provider setup before running this command.";
    case "refresh_provider_setup":
      return "Refresh provider setup state and retry the command.";
    case "enter_provider_api_key":
      return "Enter a valid provider API key and submit setup again.";
    case "recover_provider_setup":
      return "Delete and recreate provider setup if refresh does not recover it.";
    case "retry":
      return "Retry after the local service or provider becomes available.";
    case "reselect_placement":
      return "Update the placement selection and try again.";
    case "retry_provider_inventory":
      return "Reload provider inventory before creating the workspace.";
    case "reload_workflow_presets":
      return "Reload the workflow catalog, then rebuild the placement.";
    case "recover_workspace_catalog":
      return "Recover or recreate the local workspace catalog before continuing.";
    case "refresh_workspace_catalog":
      return "Refresh the workspace catalog before creating another workspace.";
    case "refresh_workspace":
      return "Refresh the workspace catalog, then retry the provisioning command.";
    case "change_request":
      return "Change the highlighted request value and retry.";
    case "inspect_workspace_provisioning":
      return "Inspect the workspace provisioning state before retrying.";
    case "cleanup_workspace_resources":
      return "Clean up workspace resources before retrying.";
    default:
      return "Review the request and retry.";
  }
}
