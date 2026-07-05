use serde::{Deserialize, Serialize};

use super::{catalog, generated};

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

impl From<generated::Reference> for Reference {
    fn from(value: generated::Reference) -> Self {
        Self {
            entity: value.entity.into(),
            id: value.id.into(),
            revision: value.revision.into(),
        }
    }
}

impl From<generated::ExecutionSchemaInputsItem> for ExecutionSchemaInput {
    fn from(value: generated::ExecutionSchemaInputsItem) -> Self {
        let input_type = value.type_.as_str();
        assert!(
            input_type.is_some(),
            "execution schema input type must be a string, got {}",
            value.type_
        );

        Self {
            id: value.id.into(),
            input_type: input_type.map(str::to_owned).unwrap_or_default(),
            required: value.required,
            max_length: value.max_length.map(Into::into),
        }
    }
}

impl From<generated::ExecutionSchemaOutputs> for ExecutionSchemaOutputs {
    fn from(value: generated::ExecutionSchemaOutputs) -> Self {
        Self {
            output_type: value.type_.into(),
        }
    }
}

impl From<catalog::ExecutionSchemaEntry> for ExecutionSchemaRevision {
    fn from(value: catalog::ExecutionSchemaEntry) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            inputs: value
                .execution_schema
                .inputs
                .into_iter()
                .map(ExecutionSchemaInput::from)
                .collect(),
            outputs: value.execution_schema.outputs.into(),
        }
    }
}

impl From<catalog::RuntimeContractEntry> for RuntimeContractRevision {
    fn from(value: catalog::RuntimeContractEntry) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            image_ref: value.runtime_contract.image_ref.into(),
        }
    }
}

impl From<generated::RuntimePresetRuntimePytorch> for RuntimePresetPytorch {
    fn from(value: generated::RuntimePresetRuntimePytorch) -> Self {
        Self {
            index_url: value.index_url.into(),
            packages: value.packages.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<generated::RuntimePresetRuntime> for RuntimePresetRuntime {
    fn from(value: generated::RuntimePresetRuntime) -> Self {
        Self {
            python_version: value.python_version.into(),
            comfyui_revision: value.comfyui_revision.into(),
            pytorch: value.pytorch.into(),
        }
    }
}

impl From<catalog::RuntimePresetEntry> for RuntimePresetRevision {
    fn from(value: catalog::RuntimePresetEntry) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            runtime: value.runtime_preset.runtime.into(),
        }
    }
}

impl From<generated::WorkflowModelAssetsModelAssetsItemDownloadSource> for ModelAssetSource {
    fn from(value: generated::WorkflowModelAssetsModelAssetsItemDownloadSource) -> Self {
        match value {
            generated::WorkflowModelAssetsModelAssetsItemDownloadSource::Huggingface {
                repository_id,
                file_path,
                revision,
            } => Self::Huggingface {
                repository_id: repository_id.into(),
                file_path: file_path.into(),
                revision: revision.into(),
            },
        }
    }
}

impl From<generated::WorkflowModelAssetsModelAssetsItem> for ModelAsset {
    fn from(value: generated::WorkflowModelAssetsModelAssetsItem) -> Self {
        Self {
            id: value.id.into(),
            name: value.name.into(),
            download_source: value.download_source.into(),
            install_comfyui_relative_path: value.install_comfyui_relative_path.into(),
        }
    }
}

impl From<generated::WorkflowContractRequirementsContractRequirementsItem>
    for WorkflowContractRequirement
{
    fn from(value: generated::WorkflowContractRequirementsContractRequirementsItem) -> Self {
        match value {
            generated::WorkflowContractRequirementsContractRequirementsItem::Runpod {
                endpoint_contract_ref,
                provisioner_contract_ref,
            } => Self::Runpod {
                endpoint_contract_ref: endpoint_contract_ref.into(),
                provisioner_contract_ref: provisioner_contract_ref.into(),
            },
        }
    }
}

impl From<generated::WorkflowExecutionContractInputBindingsItem> for InputBinding {
    fn from(value: generated::WorkflowExecutionContractInputBindingsItem) -> Self {
        Self {
            value: value.value,
            node_id: value.node_id.into(),
            path: value.path.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<generated::WorkflowExecutionContract> for WorkflowExecutionContract {
    fn from(value: generated::WorkflowExecutionContract) -> Self {
        Self {
            schema_ref: value.schema_ref.into(),
            input_bindings: value
                .input_bindings
                .into_iter()
                .map(InputBinding::from)
                .collect(),
        }
    }
}

impl From<catalog::WorkflowEntry> for WorkflowRevision {
    fn from(value: catalog::WorkflowEntry) -> Self {
        Self {
            id: value.id,
            revision: value.revision,
            name: value.metadata.name.into(),
            runtime_preset_ref: value.metadata.runtime_preset_ref.into(),
            requires_hugging_face_api_key: value.metadata.requires_hugging_face_api_key,
            required_volume_size_gb: value.metadata.required_volume_size_gb.into(),
            model_assets: value
                .model_assets
                .model_assets
                .into_iter()
                .map(ModelAsset::from)
                .collect(),
            contract_requirements: value
                .contract_requirements
                .contract_requirements
                .into_iter()
                .map(WorkflowContractRequirement::from)
                .collect(),
            execution_contract: value.execution_contract.into(),
            workflow_graph: serde_json::Value::Object(value.workflow_graph.graph),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{generated, ExecutionSchemaInput};

    #[test]
    #[should_panic(expected = "execution schema input type must be a string")]
    fn execution_schema_input_from_panics_for_non_string_type() {
        let value = generated::ExecutionSchemaInputsItem {
            id: "prompt".parse().expect("input id should parse"),
            max_length: None,
            required: true,
            type_: serde_json::json!({"type": "prompt"}),
        };

        let _ = ExecutionSchemaInput::from(value);
    }
}
