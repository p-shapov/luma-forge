use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{
    runtime_contract::RuntimeContractReference,
    workflow_preset::{
        ModelAsset, ModelAssetSource, RunpodRuntimeRequirements, WorkflowCatalog,
        WorkflowExecutionType, WorkflowPreset, WorkflowRevision,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCatalogResponse {
    pub workflow_presets: Vec<WorkflowPresetResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPresetResponse {
    pub id: String,
    pub name: String,
    pub execution_type: WorkflowExecutionTypeDto,
    pub revisions: Vec<WorkflowRevisionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRevisionResponse {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub runpod_runtime_requirements: RunpodRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum WorkflowExecutionTypeDto {
    #[serde(rename = "t2i")]
    T2i,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodRuntimeRequirementsResponse {
    pub endpoint_contract: RuntimeContractReferenceResponse,
    pub provisioner_contract: RuntimeContractReferenceResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContractReferenceResponse {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelAssetResponse {
    pub id: String,
    pub name: String,
    pub download_source: ModelAssetSourceResponse,
    pub install_comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "sourceType", rename_all = "snake_case")]
pub enum ModelAssetSourceResponse {
    Huggingface {
        repository_id: String,
        file_path: String,
        revision: String,
    },
}

impl From<WorkflowCatalog> for WorkflowCatalogResponse {
    fn from(value: WorkflowCatalog) -> Self {
        Self {
            workflow_presets: value.workflow_presets.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WorkflowPreset> for WorkflowPresetResponse {
    fn from(value: WorkflowPreset) -> Self {
        Self {
            id: value.id,
            name: value.name,
            execution_type: value.execution_type.into(),
            revisions: value.revisions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WorkflowRevision> for WorkflowRevisionResponse {
    fn from(value: WorkflowRevision) -> Self {
        Self {
            version: value.version,
            requires_hugging_face_api_key: value.requires_hugging_face_api_key,
            required_volume_size_gb: value.required_volume_size_gb,
            runpod_runtime_requirements: value.runpod_runtime_requirements.into(),
            required_model_assets: value
                .required_model_assets
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<WorkflowExecutionType> for WorkflowExecutionTypeDto {
    fn from(value: WorkflowExecutionType) -> Self {
        match value {
            WorkflowExecutionType::T2i => Self::T2i,
        }
    }
}

impl From<RunpodRuntimeRequirements> for RunpodRuntimeRequirementsResponse {
    fn from(value: RunpodRuntimeRequirements) -> Self {
        Self {
            endpoint_contract: value.endpoint_contract.into(),
            provisioner_contract: value.provisioner_contract.into(),
        }
    }
}

impl From<RuntimeContractReference> for RuntimeContractReferenceResponse {
    fn from(value: RuntimeContractReference) -> Self {
        Self {
            id: value.id,
            version: value.version,
        }
    }
}

impl From<ModelAsset> for ModelAssetResponse {
    fn from(value: ModelAsset) -> Self {
        Self {
            id: value.id,
            name: value.name,
            download_source: value.download_source.into(),
            install_comfyui_relative_path: value.install_comfyui_relative_path,
        }
    }
}

impl From<ModelAssetSource> for ModelAssetSourceResponse {
    fn from(value: ModelAssetSource) -> Self {
        match value {
            ModelAssetSource::Huggingface {
                repository_id,
                file_path,
                revision,
            } => Self::Huggingface {
                repository_id,
                file_path,
                revision,
            },
        }
    }
}
