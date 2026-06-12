use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{
    runtime_contract::RuntimeContractReference,
    workflow_preset::{
        ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements, RemoteRuntimeRequirements,
        WorkflowCatalog, WorkflowExecutionType, WorkflowPreset, WorkflowPresetResolved,
        WorkflowRevision,
    },
};

use super::provider::GpuCloudProviderIdDto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GetProviderPlacementOptionsRequest {
    pub provider_id: GpuCloudProviderIdDto,
}

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
    pub remote_runtime_requirements: RemoteRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPresetResolvedResponse {
    pub id: String,
    pub version: String,
    pub name: String,
    pub execution_type: WorkflowExecutionTypeDto,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirementsResponse,
    pub required_model_assets: Vec<ModelAssetResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum WorkflowExecutionTypeDto {
    #[serde(rename = "t2i")]
    T2i,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRuntimeRequirementsResponse {
    pub required_base_volume_size_bytes: u64,
    pub provider_requirements: Vec<RemoteProviderRuntimeRequirementsResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProviderRuntimeRequirementsResponse {
    pub gpu_cloud_provider_id: GpuCloudProviderIdDto,
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
            remote_runtime_requirements: value.remote_runtime_requirements.into(),
            required_model_assets: value
                .required_model_assets
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<WorkflowPresetResolved> for WorkflowPresetResolvedResponse {
    fn from(value: WorkflowPresetResolved) -> Self {
        Self {
            id: value.id,
            version: value.version,
            name: value.name,
            execution_type: value.execution_type.into(),
            requires_hugging_face_api_key: value.requires_hugging_face_api_key,
            remote_runtime_requirements: value.remote_runtime_requirements.into(),
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

impl From<RemoteRuntimeRequirements> for RemoteRuntimeRequirementsResponse {
    fn from(value: RemoteRuntimeRequirements) -> Self {
        Self {
            required_base_volume_size_bytes: value.required_base_volume_size_bytes,
            provider_requirements: value
                .provider_requirements
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<RemoteProviderRuntimeRequirements> for RemoteProviderRuntimeRequirementsResponse {
    fn from(value: RemoteProviderRuntimeRequirements) -> Self {
        Self {
            gpu_cloud_provider_id: value.gpu_cloud_provider_id.into(),
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
