import type { PlacementPlan } from "./placement-plan";
import type { GpuCloudProviderId } from "./provider-setup";
import type {
  PersistentStorageVolumeSnapshot,
  ProvisioningPodSnapshot,
  ServerlessEndpointSnapshot,
} from "./provider-resource";

export type WorkspaceLifecycleState =
  | "draft"
  | "provisioning"
  | "ready"
  | "failed";

export type Workspace = {
  gpu_cloud_provider_id: GpuCloudProviderId;
  id: string;
  name: string;
  lifecycle_state: WorkspaceLifecycleState;
  placement_plan: PlacementPlan;
  persistent_storage_volume_snapshot: PersistentStorageVolumeSnapshot | null;
  active_provisioning_pod_snapshot: ProvisioningPodSnapshot | null;
  serverless_endpoint_snapshot: ServerlessEndpointSnapshot | null;
  last_provisioning_pod_snapshot?: ProvisioningPodSnapshot | null;
  environment_prepared_at: string | null;
};
