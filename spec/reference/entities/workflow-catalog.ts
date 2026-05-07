import type { WorkflowPreset } from "./workflow-preset";

export type WorkflowCatalog = {
  id: string;
  workflow_presets: WorkflowPreset[];
}
