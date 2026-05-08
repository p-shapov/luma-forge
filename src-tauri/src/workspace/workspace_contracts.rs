use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::{
        placement::PlacementPlan as DomainPlacementPlan,
        profiles::{
            EndpointProfile as DomainEndpointProfile,
            ProvisionerWorkerRuntime as DomainProvisionerWorkerRuntime,
            ProvisioningProfile as DomainProvisioningProfile,
        },
        provider_setup::GpuCloudProviderId as DomainGpuCloudProviderId,
        workflow::{
            ComfyUiRuntimeSource as DomainComfyUiRuntimeSource, CustomNode as DomainCustomNode,
            CustomNodeGitSource as DomainCustomNodeGitSource,
            CustomNodeInstall as DomainCustomNodeInstall, ModelAsset as DomainModelAsset,
            ModelAssetKind as DomainModelAssetKind, ModelAssetSource as DomainModelAssetSource,
            WorkflowExecutionType as DomainWorkflowExecutionType,
            WorkflowPreset as DomainWorkflowPreset,
        },
    },
    provider::runpod::{RunPodEndpointProfileConfig, RunPodProvisioningProfileConfig},
    shared_contracts::provider_contracts::GpuCloudProviderId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DockerImage {
    pub docker_image_ref: String,
    pub docker_image_digest: String,
}

impl From<&DockerImage> for crate::domain::shared::DockerImage {
    fn from(image: &DockerImage) -> Self {
        Self {
            docker_image_ref: image.docker_image_ref.clone(),
            docker_image_digest: image.docker_image_digest.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
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

impl From<&ModelAssetKind> for DomainModelAssetKind {
    fn from(kind: &ModelAssetKind) -> Self {
        match kind {
            ModelAssetKind::Checkpoint => Self::Checkpoint,
            ModelAssetKind::DiffusionModel => Self::DiffusionModel,
            ModelAssetKind::Vae => Self::Vae,
            ModelAssetKind::TextEncoder => Self::TextEncoder,
            ModelAssetKind::Clip => Self::Clip,
            ModelAssetKind::ClipVision => Self::ClipVision,
            ModelAssetKind::Lora => Self::Lora,
            ModelAssetKind::Controlnet => Self::Controlnet,
            ModelAssetKind::Upscaler => Self::Upscaler,
            ModelAssetKind::Embedding => Self::Embedding,
            ModelAssetKind::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum ModelAssetSource {
    Huggingface {
        repository_id: String,
        file_path: String,
        revision: String,
    },
}

impl From<&ModelAssetSource> for DomainModelAssetSource {
    fn from(source: &ModelAssetSource) -> Self {
        match source {
            ModelAssetSource::Huggingface {
                repository_id,
                file_path,
                revision,
            } => Self::Huggingface {
                repository_id: repository_id.clone(),
                file_path: file_path.clone(),
                revision: revision.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub model_asset_kind: ModelAssetKind,
    pub file_size_bytes: u64,
    pub download_source: ModelAssetSource,
}

impl From<&ModelAsset> for DomainModelAsset {
    fn from(asset: &ModelAsset) -> Self {
        Self {
            id: asset.id.clone(),
            name: asset.name.clone(),
            model_asset_kind: (&asset.model_asset_kind).into(),
            file_size_bytes: asset.file_size_bytes,
            download_source: (&asset.download_source).into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum CustomNodeGitSource {
    Git {
        repository_url: String,
        revision: String,
    },
}

impl From<&CustomNodeGitSource> for DomainCustomNodeGitSource {
    fn from(source: &CustomNodeGitSource) -> Self {
        match source {
            CustomNodeGitSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url: repository_url.clone(),
                revision: revision.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CustomNodeInstall {
    pub comfyui_custom_nodes_relative_path: String,
    pub python_requirements_path: String,
}

impl From<&CustomNodeInstall> for DomainCustomNodeInstall {
    fn from(install: &CustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path.clone(),
            python_requirements_path: install.python_requirements_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CustomNode {
    pub id: String,
    pub name: String,
    pub git_source: CustomNodeGitSource,
    pub install: CustomNodeInstall,
}

impl From<&CustomNode> for DomainCustomNode {
    fn from(node: &CustomNode) -> Self {
        Self {
            id: node.id.clone(),
            name: node.name.clone(),
            git_source: (&node.git_source).into(),
            install: (&node.install).into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionType {
    T2i,
}

impl From<&WorkflowExecutionType> for DomainWorkflowExecutionType {
    fn from(execution_type: &WorkflowExecutionType) -> Self {
        match execution_type {
            WorkflowExecutionType::T2i => Self::T2i,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "source_type", rename_all = "snake_case")]
pub enum ComfyUiRuntimeSource {
    Git {
        repository_url: String,
        revision: String,
    },
}

impl From<&ComfyUiRuntimeSource> for DomainComfyUiRuntimeSource {
    fn from(source: &ComfyUiRuntimeSource) -> Self {
        match source {
            ComfyUiRuntimeSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url: repository_url.clone(),
                revision: revision.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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

impl From<&WorkflowPreset> for DomainWorkflowPreset {
    fn from(preset: &WorkflowPreset) -> Self {
        Self {
            id: preset.id.clone(),
            version: preset.version.clone(),
            name: preset.name.clone(),
            workflow_execution_type: (&preset.workflow_execution_type).into(),
            required_base_volume_size_bytes: preset.required_base_volume_size_bytes,
            required_comfyui_source: (&preset.required_comfyui_source).into(),
            required_model_assets: preset
                .required_model_assets
                .iter()
                .map(Into::into)
                .collect(),
            required_custom_nodes: preset
                .required_custom_nodes
                .iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WorkflowCatalog {
    pub id: String,
    pub version: String,
    pub workflow_presets: Vec<WorkflowPreset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningComputeType {
    Pod,
}

impl From<&ProvisioningComputeType> for crate::domain::profiles::ProvisioningComputeType {
    fn from(compute_type: &ProvisioningComputeType) -> Self {
        match compute_type {
            ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisioningStatusEndpoint {
    pub port: u16,
    pub protocol: String,
    pub status_path: String,
}

impl From<&ProvisioningStatusEndpoint> for crate::domain::profiles::ProvisioningStatusEndpoint {
    fn from(endpoint: &ProvisioningStatusEndpoint) -> Self {
        Self {
            port: endpoint.port,
            protocol: endpoint.protocol.clone(),
            status_path: endpoint.status_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisionerWorkerRuntime {
    pub provisioner_version: String,
    pub docker_image: DockerImage,
    pub volume_mount_path: String,
    pub container_disk_bytes: u64,
    pub compute_type: ProvisioningComputeType,
    pub status_endpoint: ProvisioningStatusEndpoint,
}

impl From<&ProvisionerWorkerRuntime> for DomainProvisionerWorkerRuntime {
    fn from(runtime: &ProvisionerWorkerRuntime) -> Self {
        Self {
            provisioner_version: runtime.provisioner_version.clone(),
            docker_image: (&runtime.docker_image).into(),
            volume_mount_path: runtime.volume_mount_path.clone(),
            container_disk_bytes: runtime.container_disk_bytes,
            compute_type: (&runtime.compute_type).into(),
            status_endpoint: (&runtime.status_endpoint).into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EndpointWorkerRuntime {
    pub endpoint_worker_version: String,
    pub docker_image: DockerImage,
    pub http_port: u16,
    pub health_path: String,
    pub invoke_path: String,
}

impl From<&EndpointWorkerRuntime> for crate::domain::profiles::EndpointWorkerRuntime {
    fn from(runtime: &EndpointWorkerRuntime) -> Self {
        Self {
            endpoint_worker_version: runtime.endpoint_worker_version.clone(),
            docker_image: (&runtime.docker_image).into(),
            http_port: runtime.http_port,
            health_path: runtime.health_path.clone(),
            invoke_path: runtime.invoke_path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum ProvisioningProfile {
    Runpod {
        id: String,
        version: String,
        name: String,
        provisioner_worker_runtime: ProvisionerWorkerRuntime,
        gpu_cloud_provider_config: RunPodProvisioningProfileConfig,
    },
}

impl ProvisioningProfile {
    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }

    pub fn to_domain(&self) -> DomainProvisioningProfile<RunPodProvisioningProfileConfig> {
        match self {
            Self::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
            } => DomainProvisioningProfile {
                gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
                id: id.clone(),
                version: version.clone(),
                name: name.clone(),
                provisioner_worker_runtime: provisioner_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
pub enum EndpointProfile {
    Runpod {
        id: String,
        version: String,
        name: String,
        workflow_execution_type: WorkflowExecutionType,
        endpoint_worker_runtime: EndpointWorkerRuntime,
        gpu_cloud_provider_config: RunPodEndpointProfileConfig,
    },
}

impl EndpointProfile {
    pub fn id(&self) -> &str {
        match self {
            Self::Runpod { id, .. } => id,
        }
    }

    pub fn to_domain(&self) -> DomainEndpointProfile<RunPodEndpointProfileConfig> {
        match self {
            Self::Runpod {
                id,
                version,
                name,
                workflow_execution_type,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
            } => DomainEndpointProfile {
                gpu_cloud_provider_id: DomainGpuCloudProviderId::Runpod,
                id: id.clone(),
                version: version.clone(),
                name: name.clone(),
                workflow_execution_type: workflow_execution_type.into(),
                endpoint_worker_runtime: endpoint_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PlacementPlan {
    pub selected_datacenter_id: String,
    pub selected_gpu_id: String,
    pub persistent_storage_volume_size_bytes: u64,
    pub selected_workflow_preset: WorkflowPreset,
    pub selected_provisioning_profile: ProvisioningProfile,
    pub selected_endpoint_profile: EndpointProfile,
}

impl PlacementPlan {
    pub fn to_domain(
        &self,
    ) -> DomainPlacementPlan<
        DomainProvisioningProfile<RunPodProvisioningProfileConfig>,
        DomainEndpointProfile<RunPodEndpointProfileConfig>,
    > {
        DomainPlacementPlan {
            selected_datacenter_id: self.selected_datacenter_id.clone(),
            selected_gpu_id: self.selected_gpu_id.clone(),
            persistent_storage_volume_size_bytes: self.persistent_storage_volume_size_bytes,
            selected_workflow_preset: (&self.selected_workflow_preset).into(),
            selected_provisioning_profile: self.selected_provisioning_profile.to_domain(),
            selected_endpoint_profile: self.selected_endpoint_profile.to_domain(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLifecycleState {
    Draft,
    Provisioning,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResourceStatus {
    Creating,
    Running,
    Ready,
    Terminated,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PersistentStorageVolumeSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioned_size_bytes: u64,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisioningPodSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub provisioning_profile_id: String,
    pub provisioner_status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ServerlessEndpointSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub endpoint_profile_id: String,
    pub endpoint_invoke_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Workspace {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub lifecycle_state: WorkspaceLifecycleState,
    pub placement_plan: PlacementPlan,
    pub persistent_storage_volume_snapshot: Option<PersistentStorageVolumeSnapshot>,
    pub active_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    pub serverless_endpoint_snapshot: Option<ServerlessEndpointSnapshot>,
    pub last_provisioning_pod_snapshot: Option<ProvisioningPodSnapshot>,
    pub environment_prepared_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}
