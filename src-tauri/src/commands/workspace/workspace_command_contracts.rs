use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    commands::provider_contracts::GpuCloudProviderId,
    provider::runpod,
    workspace::{
        workspace_contracts as application_workspace_contracts,
        workspace_setup_contracts as application_setup_contracts,
        workspace_setup_error::WorkspaceSetupError,
    },
};

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

impl From<ModelAssetKind> for application_workspace_contracts::ModelAssetKind {
    fn from(kind: ModelAssetKind) -> Self {
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

impl From<application_workspace_contracts::ModelAssetKind> for ModelAssetKind {
    fn from(kind: application_workspace_contracts::ModelAssetKind) -> Self {
        match kind {
            application_workspace_contracts::ModelAssetKind::Checkpoint => Self::Checkpoint,
            application_workspace_contracts::ModelAssetKind::DiffusionModel => Self::DiffusionModel,
            application_workspace_contracts::ModelAssetKind::Vae => Self::Vae,
            application_workspace_contracts::ModelAssetKind::TextEncoder => Self::TextEncoder,
            application_workspace_contracts::ModelAssetKind::Clip => Self::Clip,
            application_workspace_contracts::ModelAssetKind::ClipVision => Self::ClipVision,
            application_workspace_contracts::ModelAssetKind::Lora => Self::Lora,
            application_workspace_contracts::ModelAssetKind::Controlnet => Self::Controlnet,
            application_workspace_contracts::ModelAssetKind::Upscaler => Self::Upscaler,
            application_workspace_contracts::ModelAssetKind::Embedding => Self::Embedding,
            application_workspace_contracts::ModelAssetKind::Other => Self::Other,
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

impl From<ModelAssetSource> for application_workspace_contracts::ModelAssetSource {
    fn from(source: ModelAssetSource) -> Self {
        match source {
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

impl From<application_workspace_contracts::ModelAssetSource> for ModelAssetSource {
    fn from(source: application_workspace_contracts::ModelAssetSource) -> Self {
        match source {
            application_workspace_contracts::ModelAssetSource::Huggingface {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub model_asset_kind: ModelAssetKind,
    pub file_size_bytes: u64,
    pub download_source: ModelAssetSource,
    pub install: ModelAssetInstall,
}

impl From<ModelAsset> for application_workspace_contracts::ModelAsset {
    fn from(asset: ModelAsset) -> Self {
        Self {
            id: asset.id,
            name: asset.name,
            model_asset_kind: asset.model_asset_kind.into(),
            file_size_bytes: asset.file_size_bytes,
            download_source: asset.download_source.into(),
            install: asset.install.into(),
        }
    }
}

impl From<application_workspace_contracts::ModelAsset> for ModelAsset {
    fn from(asset: application_workspace_contracts::ModelAsset) -> Self {
        Self {
            id: asset.id,
            name: asset.name,
            model_asset_kind: asset.model_asset_kind.into(),
            file_size_bytes: asset.file_size_bytes,
            download_source: asset.download_source.into(),
            install: asset.install.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ModelAssetInstall {
    pub comfyui_relative_path: String,
}

impl From<ModelAssetInstall> for application_workspace_contracts::ModelAssetInstall {
    fn from(install: ModelAssetInstall) -> Self {
        Self {
            comfyui_relative_path: install.comfyui_relative_path,
        }
    }
}

impl From<application_workspace_contracts::ModelAssetInstall> for ModelAssetInstall {
    fn from(install: application_workspace_contracts::ModelAssetInstall) -> Self {
        Self {
            comfyui_relative_path: install.comfyui_relative_path,
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

impl From<CustomNodeGitSource> for application_workspace_contracts::CustomNodeGitSource {
    fn from(source: CustomNodeGitSource) -> Self {
        match source {
            CustomNodeGitSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
            },
        }
    }
}

impl From<application_workspace_contracts::CustomNodeGitSource> for CustomNodeGitSource {
    fn from(source: application_workspace_contracts::CustomNodeGitSource) -> Self {
        match source {
            application_workspace_contracts::CustomNodeGitSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CustomNodeInstall {
    pub comfyui_custom_nodes_relative_path: String,
    pub python_requirements_path: Option<String>,
}

impl From<CustomNodeInstall> for application_workspace_contracts::CustomNodeInstall {
    fn from(install: CustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path,
            python_requirements_path: install.python_requirements_path,
        }
    }
}

impl From<application_workspace_contracts::CustomNodeInstall> for CustomNodeInstall {
    fn from(install: application_workspace_contracts::CustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path,
            python_requirements_path: install.python_requirements_path,
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

impl From<CustomNode> for application_workspace_contracts::CustomNode {
    fn from(node: CustomNode) -> Self {
        Self {
            id: node.id,
            name: node.name,
            git_source: node.git_source.into(),
            install: node.install.into(),
        }
    }
}

impl From<application_workspace_contracts::CustomNode> for CustomNode {
    fn from(node: application_workspace_contracts::CustomNode) -> Self {
        Self {
            id: node.id,
            name: node.name,
            git_source: node.git_source.into(),
            install: node.install.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionType {
    T2i,
}

impl From<WorkflowExecutionType> for application_workspace_contracts::WorkflowExecutionType {
    fn from(execution_type: WorkflowExecutionType) -> Self {
        match execution_type {
            WorkflowExecutionType::T2i => Self::T2i,
        }
    }
}

impl From<application_workspace_contracts::WorkflowExecutionType> for WorkflowExecutionType {
    fn from(execution_type: application_workspace_contracts::WorkflowExecutionType) -> Self {
        match execution_type {
            application_workspace_contracts::WorkflowExecutionType::T2i => Self::T2i,
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

impl From<ComfyUiRuntimeSource> for application_workspace_contracts::ComfyUiRuntimeSource {
    fn from(source: ComfyUiRuntimeSource) -> Self {
        match source {
            ComfyUiRuntimeSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
            },
        }
    }
}

impl From<application_workspace_contracts::ComfyUiRuntimeSource> for ComfyUiRuntimeSource {
    fn from(source: application_workspace_contracts::ComfyUiRuntimeSource) -> Self {
        match source {
            application_workspace_contracts::ComfyUiRuntimeSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
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

impl From<WorkflowPreset> for application_workspace_contracts::WorkflowPreset {
    fn from(preset: WorkflowPreset) -> Self {
        Self {
            id: preset.id,
            version: preset.version,
            name: preset.name,
            workflow_execution_type: preset.workflow_execution_type.into(),
            required_base_volume_size_bytes: preset.required_base_volume_size_bytes,
            required_comfyui_source: preset.required_comfyui_source.into(),
            required_model_assets: preset
                .required_model_assets
                .into_iter()
                .map(Into::into)
                .collect(),
            required_custom_nodes: preset
                .required_custom_nodes
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<application_workspace_contracts::WorkflowPreset> for WorkflowPreset {
    fn from(preset: application_workspace_contracts::WorkflowPreset) -> Self {
        Self {
            id: preset.id,
            version: preset.version,
            name: preset.name,
            workflow_execution_type: preset.workflow_execution_type.into(),
            required_base_volume_size_bytes: preset.required_base_volume_size_bytes,
            required_comfyui_source: preset.required_comfyui_source.into(),
            required_model_assets: preset
                .required_model_assets
                .into_iter()
                .map(Into::into)
                .collect(),
            required_custom_nodes: preset
                .required_custom_nodes
                .into_iter()
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

impl From<application_workspace_contracts::WorkflowCatalog> for WorkflowCatalog {
    fn from(catalog: application_workspace_contracts::WorkflowCatalog) -> Self {
        Self {
            id: catalog.id,
            version: catalog.version,
            workflow_presets: catalog
                .workflow_presets
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningComputeType {
    Pod,
}

impl From<ProvisioningComputeType> for application_workspace_contracts::ProvisioningComputeType {
    fn from(compute_type: ProvisioningComputeType) -> Self {
        match compute_type {
            ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

impl From<application_workspace_contracts::ProvisioningComputeType> for ProvisioningComputeType {
    fn from(compute_type: application_workspace_contracts::ProvisioningComputeType) -> Self {
        match compute_type {
            application_workspace_contracts::ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisioningStatusEndpoint {
    pub port: u16,
    pub protocol: String,
    pub status_path: String,
}

impl From<ProvisioningStatusEndpoint>
    for application_workspace_contracts::ProvisioningStatusEndpoint
{
    fn from(endpoint: ProvisioningStatusEndpoint) -> Self {
        Self {
            port: endpoint.port,
            protocol: endpoint.protocol,
            status_path: endpoint.status_path,
        }
    }
}

impl From<application_workspace_contracts::ProvisioningStatusEndpoint>
    for ProvisioningStatusEndpoint
{
    fn from(endpoint: application_workspace_contracts::ProvisioningStatusEndpoint) -> Self {
        Self {
            port: endpoint.port,
            protocol: endpoint.protocol,
            status_path: endpoint.status_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisionerWorkerRuntime {
    pub provisioner_version: String,
    pub docker_image_ref: String,
    pub volume_mount_path: String,
    pub container_disk_bytes: u64,
    pub compute_type: ProvisioningComputeType,
    pub status_endpoint: ProvisioningStatusEndpoint,
}

impl From<ProvisionerWorkerRuntime> for application_workspace_contracts::ProvisionerWorkerRuntime {
    fn from(runtime: ProvisionerWorkerRuntime) -> Self {
        Self {
            provisioner_version: runtime.provisioner_version,
            docker_image_ref: runtime.docker_image_ref,
            volume_mount_path: runtime.volume_mount_path,
            container_disk_bytes: runtime.container_disk_bytes,
            compute_type: runtime.compute_type.into(),
            status_endpoint: runtime.status_endpoint.into(),
        }
    }
}

impl From<application_workspace_contracts::ProvisionerWorkerRuntime> for ProvisionerWorkerRuntime {
    fn from(runtime: application_workspace_contracts::ProvisionerWorkerRuntime) -> Self {
        Self {
            provisioner_version: runtime.provisioner_version,
            docker_image_ref: runtime.docker_image_ref,
            volume_mount_path: runtime.volume_mount_path,
            container_disk_bytes: runtime.container_disk_bytes,
            compute_type: runtime.compute_type.into(),
            status_endpoint: runtime.status_endpoint.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EndpointWorkerRuntime {
    pub endpoint_worker_version: String,
    pub docker_image_ref: String,
    pub http_port: u16,
    pub health_path: String,
    pub invoke_path: String,
}

impl From<EndpointWorkerRuntime> for application_workspace_contracts::EndpointWorkerRuntime {
    fn from(runtime: EndpointWorkerRuntime) -> Self {
        Self {
            endpoint_worker_version: runtime.endpoint_worker_version,
            docker_image_ref: runtime.docker_image_ref,
            http_port: runtime.http_port,
            health_path: runtime.health_path,
            invoke_path: runtime.invoke_path,
        }
    }
}

impl From<application_workspace_contracts::EndpointWorkerRuntime> for EndpointWorkerRuntime {
    fn from(runtime: application_workspace_contracts::EndpointWorkerRuntime) -> Self {
        Self {
            endpoint_worker_version: runtime.endpoint_worker_version,
            docker_image_ref: runtime.docker_image_ref,
            http_port: runtime.http_port,
            health_path: runtime.health_path,
            invoke_path: runtime.invoke_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodProvisioningProfileConfig {
    pub cloud_type: Option<String>,
    pub pod_template_id: Option<String>,
    pub network_volume_mount_path: String,
    pub expose_http_ports: Vec<u16>,
    pub env: Option<BTreeMap<String, String>>,
}

impl From<RunPodProvisioningProfileConfig> for runpod::RunPodProvisioningProfileConfig {
    fn from(config: RunPodProvisioningProfileConfig) -> Self {
        Self {
            cloud_type: config.cloud_type,
            pod_template_id: config.pod_template_id,
            network_volume_mount_path: config.network_volume_mount_path,
            expose_http_ports: config.expose_http_ports,
            env: config.env,
        }
    }
}

impl From<runpod::RunPodProvisioningProfileConfig> for RunPodProvisioningProfileConfig {
    fn from(config: runpod::RunPodProvisioningProfileConfig) -> Self {
        Self {
            cloud_type: config.cloud_type,
            pod_template_id: config.pod_template_id,
            network_volume_mount_path: config.network_volume_mount_path,
            expose_http_ports: config.expose_http_ports,
            env: config.env,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodServerlessScalingConfig {
    pub min_workers: u32,
    pub max_workers: u32,
    pub idle_timeout_seconds: u32,
    pub scaler_type: Option<String>,
    pub scaler_value: Option<u32>,
}

impl From<RunPodServerlessScalingConfig> for runpod::RunPodServerlessScalingConfig {
    fn from(config: RunPodServerlessScalingConfig) -> Self {
        Self {
            min_workers: config.min_workers,
            max_workers: config.max_workers,
            idle_timeout_seconds: config.idle_timeout_seconds,
            scaler_type: config.scaler_type,
            scaler_value: config.scaler_value,
        }
    }
}

impl From<runpod::RunPodServerlessScalingConfig> for RunPodServerlessScalingConfig {
    fn from(config: runpod::RunPodServerlessScalingConfig) -> Self {
        Self {
            min_workers: config.min_workers,
            max_workers: config.max_workers,
            idle_timeout_seconds: config.idle_timeout_seconds,
            scaler_type: config.scaler_type,
            scaler_value: config.scaler_value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RunPodEndpointProfileConfig {
    pub endpoint_template_id: Option<String>,
    pub container_disk_bytes: u64,
    pub volume_mount_path: String,
    pub env: Option<BTreeMap<String, String>>,
    pub scaling: RunPodServerlessScalingConfig,
}

impl From<RunPodEndpointProfileConfig> for runpod::RunPodEndpointProfileConfig {
    fn from(config: RunPodEndpointProfileConfig) -> Self {
        Self {
            endpoint_template_id: config.endpoint_template_id,
            container_disk_bytes: config.container_disk_bytes,
            volume_mount_path: config.volume_mount_path,
            env: config.env,
            scaling: config.scaling.into(),
        }
    }
}

impl From<runpod::RunPodEndpointProfileConfig> for RunPodEndpointProfileConfig {
    fn from(config: runpod::RunPodEndpointProfileConfig) -> Self {
        Self {
            endpoint_template_id: config.endpoint_template_id,
            container_disk_bytes: config.container_disk_bytes,
            volume_mount_path: config.volume_mount_path,
            env: config.env,
            scaling: config.scaling.into(),
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

impl From<ProvisioningProfile> for application_workspace_contracts::ProvisioningProfile {
    fn from(profile: ProvisioningProfile) -> Self {
        match profile {
            ProvisioningProfile::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
            } => Self::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime: provisioner_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.into(),
            },
        }
    }
}

impl From<application_workspace_contracts::ProvisioningProfile> for ProvisioningProfile {
    fn from(profile: application_workspace_contracts::ProvisioningProfile) -> Self {
        match profile {
            application_workspace_contracts::ProvisioningProfile::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime,
                gpu_cloud_provider_config,
            } => Self::Runpod {
                id,
                version,
                name,
                provisioner_worker_runtime: provisioner_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.into(),
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

impl From<EndpointProfile> for application_workspace_contracts::EndpointProfile {
    fn from(profile: EndpointProfile) -> Self {
        match profile {
            EndpointProfile::Runpod {
                id,
                version,
                name,
                workflow_execution_type,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
            } => Self::Runpod {
                id,
                version,
                name,
                workflow_execution_type: workflow_execution_type.into(),
                endpoint_worker_runtime: endpoint_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.into(),
            },
        }
    }
}

impl From<application_workspace_contracts::EndpointProfile> for EndpointProfile {
    fn from(profile: application_workspace_contracts::EndpointProfile) -> Self {
        match profile {
            application_workspace_contracts::EndpointProfile::Runpod {
                id,
                version,
                name,
                workflow_execution_type,
                endpoint_worker_runtime,
                gpu_cloud_provider_config,
            } => Self::Runpod {
                id,
                version,
                name,
                workflow_execution_type: workflow_execution_type.into(),
                endpoint_worker_runtime: endpoint_worker_runtime.into(),
                gpu_cloud_provider_config: gpu_cloud_provider_config.into(),
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

impl From<PlacementPlan> for application_workspace_contracts::PlacementPlan {
    fn from(plan: PlacementPlan) -> Self {
        Self {
            selected_datacenter_id: plan.selected_datacenter_id,
            selected_gpu_id: plan.selected_gpu_id,
            persistent_storage_volume_size_bytes: plan.persistent_storage_volume_size_bytes,
            selected_workflow_preset: plan.selected_workflow_preset.into(),
            selected_provisioning_profile: plan.selected_provisioning_profile.into(),
            selected_endpoint_profile: plan.selected_endpoint_profile.into(),
        }
    }
}

impl From<application_workspace_contracts::PlacementPlan> for PlacementPlan {
    fn from(plan: application_workspace_contracts::PlacementPlan) -> Self {
        Self {
            selected_datacenter_id: plan.selected_datacenter_id,
            selected_gpu_id: plan.selected_gpu_id,
            persistent_storage_volume_size_bytes: plan.persistent_storage_volume_size_bytes,
            selected_workflow_preset: plan.selected_workflow_preset.into(),
            selected_provisioning_profile: plan.selected_provisioning_profile.into(),
            selected_endpoint_profile: plan.selected_endpoint_profile.into(),
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

impl From<application_workspace_contracts::WorkspaceLifecycleState> for WorkspaceLifecycleState {
    fn from(state: application_workspace_contracts::WorkspaceLifecycleState) -> Self {
        match state {
            application_workspace_contracts::WorkspaceLifecycleState::Draft => Self::Draft,
            application_workspace_contracts::WorkspaceLifecycleState::Provisioning => {
                Self::Provisioning
            }
            application_workspace_contracts::WorkspaceLifecycleState::Ready => Self::Ready,
            application_workspace_contracts::WorkspaceLifecycleState::Failed => Self::Failed,
        }
    }
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

impl From<application_workspace_contracts::ProviderResourceStatus> for ProviderResourceStatus {
    fn from(status: application_workspace_contracts::ProviderResourceStatus) -> Self {
        match status {
            application_workspace_contracts::ProviderResourceStatus::Creating => Self::Creating,
            application_workspace_contracts::ProviderResourceStatus::Running => Self::Running,
            application_workspace_contracts::ProviderResourceStatus::Ready => Self::Ready,
            application_workspace_contracts::ProviderResourceStatus::Terminated => Self::Terminated,
            application_workspace_contracts::ProviderResourceStatus::Failed => Self::Failed,
            application_workspace_contracts::ProviderResourceStatus::Unknown => Self::Unknown,
        }
    }
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

impl From<application_workspace_contracts::PersistentStorageVolumeSnapshot>
    for PersistentStorageVolumeSnapshot
{
    fn from(snapshot: application_workspace_contracts::PersistentStorageVolumeSnapshot) -> Self {
        Self {
            gpu_cloud_provider_id: snapshot.gpu_cloud_provider_id.into(),
            provider_resource_id: snapshot.provider_resource_id,
            datacenter_id: snapshot.datacenter_id,
            provider_resource_status: snapshot.provider_resource_status.into(),
            provisioned_size_bytes: snapshot.provisioned_size_bytes,
            mount_path: snapshot.mount_path,
        }
    }
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

impl From<application_workspace_contracts::ProvisioningPodSnapshot> for ProvisioningPodSnapshot {
    fn from(snapshot: application_workspace_contracts::ProvisioningPodSnapshot) -> Self {
        Self {
            gpu_cloud_provider_id: snapshot.gpu_cloud_provider_id.into(),
            provider_resource_id: snapshot.provider_resource_id,
            datacenter_id: snapshot.datacenter_id,
            provider_resource_status: snapshot.provider_resource_status.into(),
            selected_gpu_id: snapshot.selected_gpu_id,
            provisioning_profile_id: snapshot.provisioning_profile_id,
            provisioner_status_url: snapshot.provisioner_status_url,
        }
    }
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

impl From<application_workspace_contracts::ServerlessEndpointSnapshot>
    for ServerlessEndpointSnapshot
{
    fn from(snapshot: application_workspace_contracts::ServerlessEndpointSnapshot) -> Self {
        Self {
            gpu_cloud_provider_id: snapshot.gpu_cloud_provider_id.into(),
            provider_resource_id: snapshot.provider_resource_id,
            datacenter_id: snapshot.datacenter_id,
            provider_resource_status: snapshot.provider_resource_status.into(),
            selected_gpu_id: snapshot.selected_gpu_id,
            endpoint_profile_id: snapshot.endpoint_profile_id,
            endpoint_invoke_url: snapshot.endpoint_invoke_url,
        }
    }
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

impl From<application_workspace_contracts::Workspace> for Workspace {
    fn from(workspace: application_workspace_contracts::Workspace) -> Self {
        Self {
            gpu_cloud_provider_id: workspace.gpu_cloud_provider_id.into(),
            id: workspace.id,
            name: workspace.name,
            lifecycle_state: workspace.lifecycle_state.into(),
            placement_plan: workspace.placement_plan.into(),
            persistent_storage_volume_snapshot: workspace
                .persistent_storage_volume_snapshot
                .map(Into::into),
            active_provisioning_pod_snapshot: workspace
                .active_provisioning_pod_snapshot
                .map(Into::into),
            serverless_endpoint_snapshot: workspace.serverless_endpoint_snapshot.map(Into::into),
            last_provisioning_pod_snapshot: workspace
                .last_provisioning_pod_snapshot
                .map(Into::into),
            environment_prepared_at: workspace.environment_prepared_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}

impl From<application_workspace_contracts::WorkspaceCatalog> for WorkspaceCatalog {
    fn from(catalog: application_workspace_contracts::WorkspaceCatalog) -> Self {
        Self {
            workspaces: catalog.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: WorkflowCatalog,
}

impl From<application_setup_contracts::GetWorkflowCatalogResponse> for GetWorkflowCatalogResponse {
    fn from(response: application_setup_contracts::GetWorkflowCatalogResponse) -> Self {
        Self {
            workflow_catalog: response.workflow_catalog.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<ProvisioningProfile>,
}

impl From<application_setup_contracts::GetProvisioningProfilesResponse>
    for GetProvisioningProfilesResponse
{
    fn from(response: application_setup_contracts::GetProvisioningProfilesResponse) -> Self {
        Self {
            provisioning_profiles: response
                .provisioning_profiles
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<EndpointProfile>,
}

impl From<application_setup_contracts::GetEndpointProfilesResponse>
    for GetEndpointProfilesResponse
{
    fn from(response: application_setup_contracts::GetEndpointProfilesResponse) -> Self {
        Self {
            endpoint_profiles: response
                .endpoint_profiles
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

impl TryFrom<GetProviderInventoryRequest>
    for application_setup_contracts::GetProviderInventoryRequest
{
    type Error = WorkspaceSetupError;

    fn try_from(request: GetProviderInventoryRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            gpu_cloud_provider_id: request.gpu_cloud_provider_id.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct GpuOption {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

impl From<application_setup_contracts::GpuOption> for GpuOption {
    fn from(option: application_setup_contracts::GpuOption) -> Self {
        Self {
            gpu_cloud_provider_id: option.gpu_cloud_provider_id.into(),
            id: option.id,
            name: option.name,
            vram_bytes: option.vram_bytes,
            availability_score: option.availability_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Datacenter {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub gpu_options: Vec<GpuOption>,
}

impl From<application_setup_contracts::Datacenter> for Datacenter {
    fn from(datacenter: application_setup_contracts::Datacenter) -> Self {
        Self {
            gpu_cloud_provider_id: datacenter.gpu_cloud_provider_id.into(),
            id: datacenter.id,
            name: datacenter.name,
            gpu_options: datacenter.gpu_options.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProviderInventory {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub fetched_at: String,
    pub max_persistent_storage_volume_size_bytes: Option<u64>,
    pub datacenters: Vec<Datacenter>,
}

impl From<application_setup_contracts::ProviderInventory> for ProviderInventory {
    fn from(inventory: application_setup_contracts::ProviderInventory) -> Self {
        Self {
            gpu_cloud_provider_id: inventory.gpu_cloud_provider_id.into(),
            fetched_at: inventory.fetched_at,
            max_persistent_storage_volume_size_bytes: inventory
                .max_persistent_storage_volume_size_bytes,
            datacenters: inventory.datacenters.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryResponse {
    pub provider_inventory: ProviderInventory,
}

impl From<application_setup_contracts::GetProviderInventoryResponse>
    for GetProviderInventoryResponse
{
    fn from(response: application_setup_contracts::GetProviderInventoryResponse) -> Self {
        Self {
            provider_inventory: response.provider_inventory.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: WorkspaceCatalog,
}

impl From<application_setup_contracts::GetWorkspaceCatalogResponse>
    for GetWorkspaceCatalogResponse
{
    fn from(response: application_setup_contracts::GetWorkspaceCatalogResponse) -> Self {
        Self {
            workspace_catalog: response.workspace_catalog.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}

impl TryFrom<CreateWorkspaceRequest> for application_setup_contracts::CreateWorkspaceRequest {
    type Error = WorkspaceSetupError;

    fn try_from(request: CreateWorkspaceRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            workspace_id: request.workspace_id,
            name: request.name,
            gpu_cloud_provider_id: request.gpu_cloud_provider_id.into(),
            placement_plan: request.placement_plan.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceResponse {
    pub workspace: Workspace,
}

impl From<application_setup_contracts::CreateWorkspaceResponse> for CreateWorkspaceResponse {
    fn from(response: application_setup_contracts::CreateWorkspaceResponse) -> Self {
        Self {
            workspace: response.workspace.into(),
        }
    }
}

#[cfg(test)]
#[path = "workspace_command_contract_tests.rs"]
mod workspace_command_contract_tests;
