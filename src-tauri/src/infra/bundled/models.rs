use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub entity: String,
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSchemaRevision {
    pub id: String,
    pub revision: String,
    pub inputs: Vec<ExecutionSchemaInput>,
    pub outputs: ExecutionSchemaOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchemaInput {
    pub id: String,
    pub input_type: String,
    pub required: bool,
    pub max_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchemaOutputs {
    pub output_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContractRevision {
    pub id: String,
    pub revision: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetRevision {
    pub id: String,
    pub revision: String,
    pub runtime: RuntimePresetRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetRuntime {
    pub python_version: String,
    pub comfyui_revision: String,
    pub pytorch: RuntimePresetPytorch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePresetPytorch {
    pub index_url: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub runtime_preset_ref: Reference,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub model_assets: Vec<ModelAsset>,
    pub contract_requirements: Vec<WorkflowContractRequirement>,
    pub execution_contract: WorkflowExecutionContract,
    pub workflow_graph: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub download_source: ModelAssetSource,
    pub install_comfyui_relative_path: String,
}

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
#[serde(tag = "runtime_type", rename_all = "snake_case")]
pub enum WorkflowContractRequirement {
    Runpod {
        endpoint_contract_ref: Reference,
        provisioner_contract_ref: Reference,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionContract {
    pub schema_ref: Reference,
    pub input_bindings: Vec<InputBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputBinding {
    pub value: serde_json::Value,
    pub node_id: String,
    pub path: Vec<String>,
}
