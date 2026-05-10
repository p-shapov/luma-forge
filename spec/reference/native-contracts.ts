import type { EndpointProfile } from "./entities/endpoint-profile";
import type { GpuCloudProviderId, GpuCloudProviderSetup } from "./entities/provider-setup";
import type { PlacementPlan } from "./entities/placement-plan";
import type { ProviderInventory } from "./entities/provider-inventory";
import type { WorkspaceProvisioningProgress } from "./entities/workspace-provisioning-progress";
import type { ProvisioningProfile } from "./entities/provisioning-profile";
import type { WorkflowCatalog } from "./entities/workflow-catalog";
import type { Workspace } from "./entities/workspace";
import type { WorkspaceCatalog } from "./entities/workspace-catalog";
import type { NativeCommandResult } from "./shared/native-command";

export type NativeCommandErrorCode =
  | "provider_setup_incomplete"
  | "provider_setup_not_found"
  | "provider_setup_already_exists"
  | "provider_api_key_required"
  | "provider_api_key_unauthorized"
  | "stored_provider_api_key_invalid"
  | "provider_api_unavailable"
  | "provider_response_invalid"
  | "provider_inventory_invalid"
  | "provider_identity_response_invalid"
  | "secure_keyring_unavailable"
  | "provider_setup_recovery_required"
  | "workflow_catalog_unavailable"
  | "provisioning_profiles_unavailable"
  | "endpoint_profiles_unavailable"
  | "workspace_catalog_unavailable"
  | "workspace_catalog_storage_unavailable"
  | "workspace_catalog_migration_failed"
  | "workspace_catalog_query_failed"
  | "workspace_catalog_corrupt"
  | "workspace_catalog_schema_mismatch"
  | "placement_provider_mismatch"
  | "placement_datacenter_required"
  | "placement_gpu_required"
  | "workflow_preset_stale"
  | "provisioning_profile_stale"
  | "endpoint_profile_stale"
  | "endpoint_profile_incompatible"
  | "storage_size_below_preset_minimum"
  | "workspace_already_exists"
  | "invalid_workspace_id"
  | "workspace_name_required"
  | "invalid_workspace_metadata";

export type NativeCommandError = {
  code: NativeCommandErrorCode;
  message: string;
  retryable: boolean;
  field: string | null;
  reason: string | null;
  recovery_action: string | null;
}

export type GetGpuCloudProviderSetupRequest = {
  gpu_cloud_provider_id: GpuCloudProviderId;
}

export type GetGpuCloudProviderSetupResponse = {
  gpu_cloud_provider_setup: GpuCloudProviderSetup | null;
}

export type SetupGpuCloudProviderRequest = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  provider_api_key: string;
}

export type SetupGpuCloudProviderResponse = {
  gpu_cloud_provider_setup: GpuCloudProviderSetup;
}

export type DeleteGpuCloudProviderSetupRequest = {
  gpu_cloud_provider_id: GpuCloudProviderId;
}

export type DeleteGpuCloudProviderSetupResponse = {
  gpu_cloud_provider_setup: GpuCloudProviderSetup | null;
}

export type GetWorkflowCatalogResponse = {
  workflow_catalog: WorkflowCatalog;
}

export type GetWorkspaceCatalogResponse = {
  workspace_catalog: WorkspaceCatalog;
}

export type GetProvisioningProfilesResponse = {
  provisioning_profiles: ProvisioningProfile[];
}

export type GetEndpointProfilesResponse = {
  endpoint_profiles: EndpointProfile[];
}

export type GetProviderInventoryRequest = {
  gpu_cloud_provider_id: GpuCloudProviderId;
}

export type GetProviderInventoryResponse = {
  provider_inventory: ProviderInventory;
}

export type CreateWorkspaceRequest = {
  workspace_id: string;
  name: string;
  gpu_cloud_provider_id: GpuCloudProviderId;
  placement_plan: PlacementPlan;
}

export type CreateWorkspaceResponse = {
  workspace: Workspace;
}

export type InitiateProvisioningForWorkspaceIdRequest = {
  workspace_id: string;
}

export type InitiateProvisioningForWorkspaceIdResult = {
  workspace: Workspace;
  provisioning: WorkspaceProvisioningProgress;
}

export type CancelProvisioningForWorkspaceIdRequest = {
  workspace_id: string;
}

export type CancelProvisioningForWorkspaceIdResponse = {
  workspace: Workspace;
  provisioning: WorkspaceProvisioningProgress;
}

export type SyncProvisioningForWorkspaceIdRequest = {
  workspace_id: string;
}

export type SyncProvisioningForWorkspaceIdResponse = {
  workspace: Workspace;
  provisioning: WorkspaceProvisioningProgress;
}

export type NativeCommandApi = {
  setup_gpu_cloud_provider(
    request: SetupGpuCloudProviderRequest,
  ): NativeCommandResult<SetupGpuCloudProviderResponse>;
  
  get_gpu_cloud_provider_setup(
    request: GetGpuCloudProviderSetupRequest,
  ): NativeCommandResult<GetGpuCloudProviderSetupResponse>;

  delete_gpu_cloud_provider_setup(
    request: DeleteGpuCloudProviderSetupRequest,
  ): NativeCommandResult<DeleteGpuCloudProviderSetupResponse>;

  get_workflow_catalog(): NativeCommandResult<GetWorkflowCatalogResponse>;

  get_workspace_catalog(): NativeCommandResult<GetWorkspaceCatalogResponse>;

  get_provisioning_profiles(): NativeCommandResult<GetProvisioningProfilesResponse>;

  get_endpoint_profiles(): NativeCommandResult<GetEndpointProfilesResponse>;

  get_provider_inventory(
    request: GetProviderInventoryRequest,
  ): NativeCommandResult<GetProviderInventoryResponse>;

  create_workspace(
    request: CreateWorkspaceRequest,
  ): NativeCommandResult<CreateWorkspaceResponse>;

  initiate_provisioning_for_workspace_id(
    request: InitiateProvisioningForWorkspaceIdRequest,
  ): NativeCommandResult<InitiateProvisioningForWorkspaceIdResult>;

  cancel_provisioning_for_workspace_id(
    request: CancelProvisioningForWorkspaceIdRequest,
  ): NativeCommandResult<CancelProvisioningForWorkspaceIdResponse>;

  sync_provisioning_for_workspace_id(
    request: SyncProvisioningForWorkspaceIdRequest,
  ): NativeCommandResult<SyncProvisioningForWorkspaceIdResponse>;
}
