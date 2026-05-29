use serde::{Deserialize, Serialize};

use super::runtime_contract::RuntimeContractReference;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionType {
    T2i,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPreset {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub required_base_volume_size_bytes: u64,
    pub requires_hugging_face_api_key: bool,
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
    pub required_model_assets: Vec<ModelAsset>,
}
