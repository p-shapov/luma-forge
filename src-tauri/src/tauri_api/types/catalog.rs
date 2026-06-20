use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{
    runpod::RunpodContractRequirements,
    runtime_contract::{
        RuntimeCatalog, RuntimeContract, RuntimeContractReference, RuntimeContractRevision,
    },
    workflow_preset::{
        ModelAsset, ModelAssetSource, WorkflowCatalog, WorkflowContractRequirements,
        WorkflowPreset, WorkflowRevision,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCatalogResponse {
    pub workflow_presets: Vec<WorkflowPresetResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCatalogResponse {
    pub contracts: Vec<RuntimeContractResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContractResponse {
    pub id: String,
    pub revisions: Vec<RuntimeContractRevisionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeContractRevisionResponse {
    pub version: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPresetResponse {
    pub id: String,
    pub name: String,
    pub revisions: Vec<WorkflowRevisionResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRevisionResponse {
    pub version: String,
    pub runtime_preset: String,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub contract_requirements: Vec<WorkflowContractRequirementsResponse>,
    pub required_model_assets: Vec<ModelAssetResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "runtimeType", rename_all = "snake_case")]
pub enum WorkflowContractRequirementsResponse {
    Runpod(RunpodContractRequirementsResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunpodContractRequirementsResponse {
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

impl From<RuntimeCatalog> for RuntimeCatalogResponse {
    fn from(value: RuntimeCatalog) -> Self {
        Self {
            contracts: value.contracts.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RuntimeContract> for RuntimeContractResponse {
    fn from(value: RuntimeContract) -> Self {
        Self {
            id: value.id,
            revisions: value.revisions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<RuntimeContractRevision> for RuntimeContractRevisionResponse {
    fn from(value: RuntimeContractRevision) -> Self {
        Self {
            version: value.version,
            image_ref: value.image_ref,
        }
    }
}

impl From<WorkflowPreset> for WorkflowPresetResponse {
    fn from(value: WorkflowPreset) -> Self {
        Self {
            id: value.id,
            name: value.name,
            revisions: value.revisions.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<WorkflowRevision> for WorkflowRevisionResponse {
    fn from(value: WorkflowRevision) -> Self {
        Self {
            version: value.version,
            runtime_preset: value.runtime_preset,
            requires_hugging_face_api_key: value.requires_hugging_face_api_key,
            required_volume_size_gb: value.required_volume_size_gb,
            contract_requirements: value
                .contract_requirements
                .into_iter()
                .map(Into::into)
                .collect(),
            required_model_assets: value
                .required_model_assets
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<WorkflowContractRequirements> for WorkflowContractRequirementsResponse {
    fn from(value: WorkflowContractRequirements) -> Self {
        match value {
            WorkflowContractRequirements::Runpod(requirements) => Self::Runpod(requirements.into()),
        }
    }
}

impl From<RunpodContractRequirements> for RunpodContractRequirementsResponse {
    fn from(value: RunpodContractRequirements) -> Self {
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
