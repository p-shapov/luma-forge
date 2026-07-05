#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledReference {
    pub id: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflow {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub runtime_preset: BundledReference,
    pub requires_hugging_face_api_key: bool,
    pub required_volume_size_gb: u64,
    pub model_assets: Vec<BundledModelAsset>,
    pub contract_requirements: Vec<BundledWorkflowContractRequirement>,
    pub execution_contract: BundledWorkflowExecutionContract,
    pub graph: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledModelAsset {
    pub id: String,
    pub name: String,
    pub download_source: BundledModelAssetDownloadSource,
    pub install_comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledModelAssetDownloadSource {
    pub source_type: String,
    pub repository_id: String,
    pub file_path: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledWorkflowContractRequirement {
    pub runtime_type: String,
    pub endpoint_contract: BundledReference,
    pub provisioner_contract: BundledReference,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflowExecutionContract {
    pub schema_ref: BundledReference,
    pub input_bindings: Vec<BundledWorkflowInputBinding>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BundledWorkflowInputBinding {
    pub value: serde_json::Value,
    pub node_id: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePreset {
    pub id: String,
    pub revision: String,
    pub runtime: BundledRuntimePresetRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePresetRuntime {
    pub python_version: String,
    pub comfyui_revision: String,
    pub pytorch: BundledRuntimePresetPytorch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimePresetPytorch {
    pub index_url: String,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledRuntimeContract {
    pub id: String,
    pub revision: String,
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledExecutionSchema {
    pub id: String,
    pub revision: String,
    pub inputs: Vec<BundledExecutionInput>,
    pub output_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledExecutionInput {
    pub id: String,
    pub input_type: String,
    pub required: bool,
    pub max_length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRunpodWorkflow {
    pub workflow: BundledWorkflow,
    pub runtime_preset: BundledRuntimePreset,
    pub execution_schema: BundledExecutionSchema,
    pub endpoint_contract: BundledRuntimeContract,
    pub provisioner_contract: BundledRuntimeContract,
}
