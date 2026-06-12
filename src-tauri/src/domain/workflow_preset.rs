use serde::{Deserialize, Serialize};

use super::{provisioned_remote::GpuCloudProviderId, runtime_contract::RuntimeContractReference};

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
pub struct RemoteProviderRuntimeRequirements {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub endpoint_contract: RuntimeContractReference,
    pub provisioner_contract: RuntimeContractReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRuntimeRequirements {
    pub required_base_volume_size_bytes: u64,
    pub provider_requirements: Vec<RemoteProviderRuntimeRequirements>,
}

impl RemoteRuntimeRequirements {
    pub fn resolve_provider_requirements(
        &self,
        gpu_cloud_provider_id: GpuCloudProviderId,
    ) -> Option<&RemoteProviderRuntimeRequirements> {
        self.provider_requirements
            .iter()
            .find(|requirements| requirements.gpu_cloud_provider_id == gpu_cloud_provider_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPreset {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalog {
    pub workflow_presets: Vec<WorkflowPreset>,
}
