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
  | "unsupported_provider"
  | "provider_setup_incomplete"
  | "provider_setup_already_exists"
  | "invalid_provider_api_key"
  | "provider_api_unavailable"
  | "secure_keyring_unavailable"
  | "local_storage_unavailable"
  | "workflow_catalog_unavailable"
  | "workspace_catalog_unavailable"
  | "invalid_placement_plan"
  | "workspace_already_exists"
  | "invalid_request";

export type NativeCommandError = {
  code: NativeCommandErrorCode;
  message: string;
  retryable: boolean;
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
