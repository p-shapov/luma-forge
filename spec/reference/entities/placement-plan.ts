import type { WorkflowPreset } from "./workflow-preset";

export type PlacementPlan = {
  selected_datacenter_id: string; 
  selected_gpu_id: string;
  persistent_storage_volume_size_bytes: number;
  selected_workflow_preset: WorkflowPreset;
}
