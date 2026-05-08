#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAssetKind {
    Checkpoint,
    DiffusionModel,
    Vae,
    TextEncoder,
    Clip,
    ClipVision,
    Lora,
    Controlnet,
    Upscaler,
    Embedding,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelAssetSource {
    Huggingface {
        repository_id: String,
        file_path: String,
        revision: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub model_asset_kind: ModelAssetKind,
    pub file_size_bytes: u64,
    pub download_source: ModelAssetSource,
    pub install: ModelAssetInstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAssetInstall {
    pub comfyui_relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomNodeGitSource {
    Git {
        repository_url: String,
        revision: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomNodeInstall {
    pub comfyui_custom_nodes_relative_path: String,
    pub python_requirements_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomNode {
    pub id: String,
    pub name: String,
    pub git_source: CustomNodeGitSource,
    pub install: CustomNodeInstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowExecutionType {
    T2i,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComfyUiRuntimeSource {
    Git {
        repository_url: String,
        revision: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPreset {
    pub id: String,
    pub version: String,
    pub name: String,
    pub workflow_execution_type: WorkflowExecutionType,
    pub required_base_volume_size_bytes: u64,
    pub required_comfyui_source: ComfyUiRuntimeSource,
    pub required_model_assets: Vec<ModelAsset>,
    pub required_custom_nodes: Vec<CustomNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct WorkflowCatalog {
    pub id: String,
    pub version: String,
    pub workflow_presets: Vec<WorkflowPreset>,
}
