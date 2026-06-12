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
pub struct WorkflowReference {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRevision {
    pub version: String,
    pub requires_hugging_face_api_key: bool,
    pub remote_runtime_requirements: RemoteRuntimeRequirements,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPreset {
    pub id: String,
    pub name: String,
    pub execution_type: WorkflowExecutionType,
    pub revisions: Vec<WorkflowRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPresetResolved {
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

impl WorkflowCatalog {
    pub fn resolve(&self, reference: &WorkflowReference) -> Option<WorkflowPresetResolved> {
        let preset = self
            .workflow_presets
            .iter()
            .find(|preset| preset.id == reference.id)?;
        let revision = preset
            .revisions
            .iter()
            .find(|revision| revision.version == reference.version)?;

        Some(WorkflowPresetResolved {
            id: preset.id.clone(),
            version: revision.version.clone(),
            name: preset.name.clone(),
            execution_type: preset.execution_type,
            requires_hugging_face_api_key: revision.requires_hugging_face_api_key,
            remote_runtime_requirements: revision.remote_runtime_requirements.clone(),
            required_model_assets: revision.required_model_assets.clone(),
        })
    }

    pub fn resolve_latest(&self, preset_id: &str) -> Option<WorkflowPresetResolved> {
        let preset = self
            .workflow_presets
            .iter()
            .find(|preset| preset.id == preset_id)?;
        let revision = preset.revisions.last()?;

        Some(WorkflowPresetResolved {
            id: preset.id.clone(),
            version: revision.version.clone(),
            name: preset.name.clone(),
            execution_type: preset.execution_type,
            requires_hugging_face_api_key: revision.requires_hugging_face_api_key,
            remote_runtime_requirements: revision.remote_runtime_requirements.clone(),
            required_model_assets: revision.required_model_assets.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        provisioned_remote::GpuCloudProviderId, runtime_contract::RuntimeContractReference,
    };

    fn reference(version: &str) -> WorkflowReference {
        WorkflowReference {
            id: "workflow".to_string(),
            version: version.to_string(),
        }
    }

    fn revision(version: &str, volume_size: u64) -> WorkflowRevision {
        WorkflowRevision {
            version: version.to_string(),
            requires_hugging_face_api_key: true,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: volume_size,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "endpoint".to_string(),
                        version: "1.0.0".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner".to_string(),
                        version: "1.0.0".to_string(),
                    },
                }],
            },
            required_model_assets: Vec::new(),
        }
    }

    fn catalog() -> WorkflowCatalog {
        WorkflowCatalog {
            workflow_presets: vec![WorkflowPreset {
                id: "workflow".to_string(),
                name: "Workflow".to_string(),
                execution_type: WorkflowExecutionType::T2i,
                revisions: vec![revision("1.0.0", 1), revision("1.1.0", 2)],
            }],
        }
    }

    #[test]
    fn workflow_catalog_resolves_reference_to_revision() {
        let resolved = catalog()
            .resolve(&reference("1.1.0"))
            .expect("workflow reference should resolve");

        assert_eq!(resolved.id, "workflow");
        assert_eq!(resolved.version, "1.1.0");
        assert_eq!(resolved.name, "Workflow");
        assert_eq!(resolved.execution_type, WorkflowExecutionType::T2i);
        assert_eq!(
            resolved
                .remote_runtime_requirements
                .required_base_volume_size_bytes,
            2
        );
    }

    #[test]
    fn workflow_catalog_rejects_missing_revision() {
        assert_eq!(catalog().resolve(&reference("2.0.0")), None);
    }

    #[test]
    fn workflow_catalog_resolves_latest_revision_for_preset() {
        let resolved = catalog()
            .resolve_latest("workflow")
            .expect("latest workflow revision should resolve");

        assert_eq!(resolved.id, "workflow");
        assert_eq!(resolved.version, "1.1.0");
        assert_eq!(
            resolved
                .remote_runtime_requirements
                .required_base_volume_size_bytes,
            2
        );
    }

    #[test]
    fn workflow_catalog_rejects_latest_for_missing_preset() {
        assert_eq!(catalog().resolve_latest("missing"), None);
    }
}
