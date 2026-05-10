use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    commands::contracts::GpuCloudProviderId,
    domain::{
        placement as domain_placement, profiles as domain_profiles,
        provider_inventory as domain_inventory, workflow as domain_workflow,
        workspace as domain_workspace,
    },
    provider::runpod,
    workspace_setup::{contracts::CreateWorkspaceInput, error::WorkspaceSetupError},
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

impl From<ModelAssetKind> for domain_workflow::ModelAssetKind {
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

impl From<domain_workflow::ModelAssetKind> for ModelAssetKind {
    fn from(kind: domain_workflow::ModelAssetKind) -> Self {
        match kind {
            domain_workflow::ModelAssetKind::Checkpoint => Self::Checkpoint,
            domain_workflow::ModelAssetKind::DiffusionModel => Self::DiffusionModel,
            domain_workflow::ModelAssetKind::Vae => Self::Vae,
            domain_workflow::ModelAssetKind::TextEncoder => Self::TextEncoder,
            domain_workflow::ModelAssetKind::Clip => Self::Clip,
            domain_workflow::ModelAssetKind::ClipVision => Self::ClipVision,
            domain_workflow::ModelAssetKind::Lora => Self::Lora,
            domain_workflow::ModelAssetKind::Controlnet => Self::Controlnet,
            domain_workflow::ModelAssetKind::Upscaler => Self::Upscaler,
            domain_workflow::ModelAssetKind::Embedding => Self::Embedding,
            domain_workflow::ModelAssetKind::Other => Self::Other,
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

impl From<ModelAssetSource> for domain_workflow::ModelAssetSource {
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

impl From<domain_workflow::ModelAssetSource> for ModelAssetSource {
    fn from(source: domain_workflow::ModelAssetSource) -> Self {
        match source {
            domain_workflow::ModelAssetSource::Huggingface {
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

impl From<ModelAsset> for domain_workflow::ModelAsset {
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

impl From<domain_workflow::ModelAsset> for ModelAsset {
    fn from(asset: domain_workflow::ModelAsset) -> Self {
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

impl From<ModelAssetInstall> for domain_workflow::ModelAssetInstall {
    fn from(install: ModelAssetInstall) -> Self {
        Self {
            comfyui_relative_path: install.comfyui_relative_path,
        }
    }
}

impl From<domain_workflow::ModelAssetInstall> for ModelAssetInstall {
    fn from(install: domain_workflow::ModelAssetInstall) -> Self {
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

impl From<CustomNodeGitSource> for domain_workflow::CustomNodeGitSource {
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

impl From<domain_workflow::CustomNodeGitSource> for CustomNodeGitSource {
    fn from(source: domain_workflow::CustomNodeGitSource) -> Self {
        match source {
            domain_workflow::CustomNodeGitSource::Git {
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

impl From<CustomNodeInstall> for domain_workflow::CustomNodeInstall {
    fn from(install: CustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path,
            python_requirements_path: install.python_requirements_path,
        }
    }
}

impl From<domain_workflow::CustomNodeInstall> for CustomNodeInstall {
    fn from(install: domain_workflow::CustomNodeInstall) -> Self {
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

impl From<CustomNode> for domain_workflow::CustomNode {
    fn from(node: CustomNode) -> Self {
        Self {
            id: node.id,
            name: node.name,
            git_source: node.git_source.into(),
            install: node.install.into(),
        }
    }
}

impl From<domain_workflow::CustomNode> for CustomNode {
    fn from(node: domain_workflow::CustomNode) -> Self {
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

impl From<WorkflowExecutionType> for domain_workflow::WorkflowExecutionType {
    fn from(execution_type: WorkflowExecutionType) -> Self {
        match execution_type {
            WorkflowExecutionType::T2i => Self::T2i,
        }
    }
}

impl From<domain_workflow::WorkflowExecutionType> for WorkflowExecutionType {
    fn from(execution_type: domain_workflow::WorkflowExecutionType) -> Self {
        match execution_type {
            domain_workflow::WorkflowExecutionType::T2i => Self::T2i,
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

impl From<ComfyUiRuntimeSource> for domain_workflow::ComfyUiRuntimeSource {
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

impl From<domain_workflow::ComfyUiRuntimeSource> for ComfyUiRuntimeSource {
    fn from(source: domain_workflow::ComfyUiRuntimeSource) -> Self {
        match source {
            domain_workflow::ComfyUiRuntimeSource::Git {
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

impl From<WorkflowPreset> for domain_workflow::WorkflowPreset {
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

impl From<domain_workflow::WorkflowPreset> for WorkflowPreset {
    fn from(preset: domain_workflow::WorkflowPreset) -> Self {
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

impl From<domain_workflow::WorkflowCatalog> for WorkflowCatalog {
    fn from(catalog: domain_workflow::WorkflowCatalog) -> Self {
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

impl From<ProvisioningComputeType> for domain_profiles::ProvisioningComputeType {
    fn from(compute_type: ProvisioningComputeType) -> Self {
        match compute_type {
            ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

impl From<domain_profiles::ProvisioningComputeType> for ProvisioningComputeType {
    fn from(compute_type: domain_profiles::ProvisioningComputeType) -> Self {
        match compute_type {
            domain_profiles::ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProvisioningStatusEndpoint {
    pub port: u16,
    pub protocol: String,
    pub status_path: String,
}

impl From<ProvisioningStatusEndpoint> for domain_profiles::ProvisioningStatusEndpoint {
    fn from(endpoint: ProvisioningStatusEndpoint) -> Self {
        Self {
            port: endpoint.port,
            protocol: endpoint.protocol,
            status_path: endpoint.status_path,
        }
    }
}

impl From<domain_profiles::ProvisioningStatusEndpoint> for ProvisioningStatusEndpoint {
    fn from(endpoint: domain_profiles::ProvisioningStatusEndpoint) -> Self {
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

impl From<ProvisionerWorkerRuntime> for domain_profiles::ProvisionerWorkerRuntime {
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

impl From<domain_profiles::ProvisionerWorkerRuntime> for ProvisionerWorkerRuntime {
    fn from(runtime: domain_profiles::ProvisionerWorkerRuntime) -> Self {
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

impl From<EndpointWorkerRuntime> for domain_profiles::EndpointWorkerRuntime {
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

impl From<domain_profiles::EndpointWorkerRuntime> for EndpointWorkerRuntime {
    fn from(runtime: domain_profiles::EndpointWorkerRuntime) -> Self {
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

impl From<ProvisioningProfile> for domain_profiles::ProvisioningProfile {
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

impl From<domain_profiles::ProvisioningProfile> for ProvisioningProfile {
    fn from(profile: domain_profiles::ProvisioningProfile) -> Self {
        match profile {
            domain_profiles::ProvisioningProfile::Runpod {
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

impl From<EndpointProfile> for domain_profiles::EndpointProfile {
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

impl From<domain_profiles::EndpointProfile> for EndpointProfile {
    fn from(profile: domain_profiles::EndpointProfile) -> Self {
        match profile {
            domain_profiles::EndpointProfile::Runpod {
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

impl From<PlacementPlan> for domain_placement::PlacementPlan {
    fn from(plan: PlacementPlan) -> Self {
        Self::Runpod {
            selected_datacenter_id: plan.selected_datacenter_id,
            selected_gpu_id: plan.selected_gpu_id,
            persistent_storage_volume_size_bytes: plan.persistent_storage_volume_size_bytes,
            selected_workflow_preset: plan.selected_workflow_preset.into(),
            selected_provisioning_profile: plan.selected_provisioning_profile.into(),
            selected_endpoint_profile: plan.selected_endpoint_profile.into(),
        }
    }
}

impl From<domain_placement::PlacementPlan> for PlacementPlan {
    fn from(plan: domain_placement::PlacementPlan) -> Self {
        match plan {
            domain_placement::PlacementPlan::Runpod {
                selected_datacenter_id,
                selected_gpu_id,
                persistent_storage_volume_size_bytes,
                selected_workflow_preset,
                selected_provisioning_profile,
                selected_endpoint_profile,
            } => Self {
                selected_datacenter_id,
                selected_gpu_id,
                persistent_storage_volume_size_bytes,
                selected_workflow_preset: selected_workflow_preset.into(),
                selected_provisioning_profile: selected_provisioning_profile.into(),
                selected_endpoint_profile: selected_endpoint_profile.into(),
            },
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

impl From<domain_workspace::WorkspaceLifecycleState> for WorkspaceLifecycleState {
    fn from(state: domain_workspace::WorkspaceLifecycleState) -> Self {
        match state {
            domain_workspace::WorkspaceLifecycleState::Draft => Self::Draft,
            domain_workspace::WorkspaceLifecycleState::Provisioning => Self::Provisioning,
            domain_workspace::WorkspaceLifecycleState::Ready => Self::Ready,
            domain_workspace::WorkspaceLifecycleState::Failed => Self::Failed,
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

impl From<domain_workspace::ProviderResourceStatus> for ProviderResourceStatus {
    fn from(status: domain_workspace::ProviderResourceStatus) -> Self {
        match status {
            domain_workspace::ProviderResourceStatus::Creating => Self::Creating,
            domain_workspace::ProviderResourceStatus::Running => Self::Running,
            domain_workspace::ProviderResourceStatus::Ready => Self::Ready,
            domain_workspace::ProviderResourceStatus::Terminated => Self::Terminated,
            domain_workspace::ProviderResourceStatus::Failed => Self::Failed,
            domain_workspace::ProviderResourceStatus::Unknown => Self::Unknown,
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

impl From<domain_workspace::PersistentStorageVolumeSnapshot> for PersistentStorageVolumeSnapshot {
    fn from(snapshot: domain_workspace::PersistentStorageVolumeSnapshot) -> Self {
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

impl From<domain_workspace::ProvisioningPodSnapshot> for ProvisioningPodSnapshot {
    fn from(snapshot: domain_workspace::ProvisioningPodSnapshot) -> Self {
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

impl From<domain_workspace::ServerlessEndpointSnapshot> for ServerlessEndpointSnapshot {
    fn from(snapshot: domain_workspace::ServerlessEndpointSnapshot) -> Self {
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

impl From<domain_workspace::Workspace> for Workspace {
    fn from(workspace: domain_workspace::Workspace) -> Self {
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

impl From<domain_workspace::WorkspaceCatalog> for WorkspaceCatalog {
    fn from(catalog: domain_workspace::WorkspaceCatalog) -> Self {
        Self {
            workspaces: catalog.workspaces.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: WorkflowCatalog,
}

impl From<domain_workflow::WorkflowCatalog> for GetWorkflowCatalogResponse {
    fn from(response: domain_workflow::WorkflowCatalog) -> Self {
        Self {
            workflow_catalog: response.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<ProvisioningProfile>,
}

impl From<Vec<domain_profiles::ProvisioningProfile>> for GetProvisioningProfilesResponse {
    fn from(response: Vec<domain_profiles::ProvisioningProfile>) -> Self {
        Self {
            provisioning_profiles: response.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<EndpointProfile>,
}

impl From<Vec<domain_profiles::EndpointProfile>> for GetEndpointProfilesResponse {
    fn from(response: Vec<domain_profiles::EndpointProfile>) -> Self {
        Self {
            endpoint_profiles: response.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct GpuOption {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub id: String,
    pub name: String,
    pub vram_bytes: u64,
    pub availability_score: u8,
}

impl From<domain_inventory::GpuOption> for GpuOption {
    fn from(option: domain_inventory::GpuOption) -> Self {
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

impl From<domain_inventory::Datacenter> for Datacenter {
    fn from(datacenter: domain_inventory::Datacenter) -> Self {
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

impl From<domain_inventory::ProviderInventory> for ProviderInventory {
    fn from(inventory: domain_inventory::ProviderInventory) -> Self {
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

impl From<domain_inventory::ProviderInventory> for GetProviderInventoryResponse {
    fn from(response: domain_inventory::ProviderInventory) -> Self {
        Self {
            provider_inventory: response.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: WorkspaceCatalog,
}

impl From<domain_workspace::WorkspaceCatalog> for GetWorkspaceCatalogResponse {
    fn from(response: domain_workspace::WorkspaceCatalog) -> Self {
        Self {
            workspace_catalog: response.into(),
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

impl TryFrom<CreateWorkspaceRequest> for CreateWorkspaceInput {
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

impl From<domain_workspace::Workspace> for CreateWorkspaceResponse {
    fn from(response: domain_workspace::Workspace) -> Self {
        Self {
            workspace: response.into(),
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
