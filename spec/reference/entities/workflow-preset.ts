import type { ModelAsset } from "./model-asset";

type RuntimeContractReference = {
  id: string;
  version: string;
}

export type WorkflowExecutionType =
  | "t2i";

export type WorkflowPreset = {
  id: string;
  name: string;
  workflow_execution_type: WorkflowExecutionType;
  required_base_volume_size_bytes: number;
  required_runtime_contract: RuntimeContractReference;
  required_model_assets: ModelAsset[];
}
