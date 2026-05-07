import type { CustomNode } from "./custom-node";
import type { ModelAsset } from "./model-asset";

type ComfyUiRuntimeSource = {
  source_type: "git";
  repository_url: string;
  revision: string;
}

export type WorkflowExecutionType =
  | "t2i";

export type WorkflowPreset = {
  id: string;
  name: string;
  workflow_execution_type: WorkflowExecutionType;
  required_base_volume_size_bytes: number;
  required_comfyui_source: ComfyUiRuntimeSource;
  required_model_assets: ModelAsset[];
  required_custom_nodes: CustomNode[];
}
