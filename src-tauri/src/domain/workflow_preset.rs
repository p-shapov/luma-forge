use serde::{Deserialize, Serialize};

use crate::domain::runpod::RunpodContractRequirements;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum ModelAssetSource {
    Huggingface {
        repository_id: String,
        file_path: String,
        revision: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub download_source: ModelAssetSource,
    pub install_comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowContractRequirements {
    Runpod(RunpodContractRequirements),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchemaReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputBinding {
    pub value: serde_json::Value,
    pub node_id: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContract {
    pub schema_ref: ExecutionSchemaReference,
    pub input_bindings: Vec<InputBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub version: String,
    pub runtime_preset: String,
    pub execution_contract: ExecutionContract,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub contract_requirements: Vec<WorkflowContractRequirements>,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPreset {
    pub id: String,
    pub name: String,
    pub revisions: Vec<WorkflowRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalog {
    pub workflow_presets: Vec<WorkflowPreset>,
}
