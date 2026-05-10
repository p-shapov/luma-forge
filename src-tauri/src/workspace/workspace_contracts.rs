use serde::{Deserialize, Serialize};

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
        workspace::{
            PersistentStorageVolumeSnapshot as DomainPersistentStorageVolumeSnapshot,
            ProviderResourceStatus as DomainProviderResourceStatus,
            ProvisioningPodSnapshot as DomainProvisioningPodSnapshot,
            ServerlessEndpointSnapshot as DomainServerlessEndpointSnapshot,
            Workspace as DomainWorkspace, WorkspaceLifecycleState as DomainWorkspaceLifecycleState,
        },
    },
    provider::runpod::{RunPodEndpointProfileConfig, RunPodProvisioningProfileConfig},
    shared_contracts::provider_contracts::GpuCloudProviderId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainModelAssetKind> for ModelAssetKind {
    fn from(kind: DomainModelAssetKind) -> Self {
        match kind {
            DomainModelAssetKind::Checkpoint => Self::Checkpoint,
            DomainModelAssetKind::DiffusionModel => Self::DiffusionModel,
            DomainModelAssetKind::Vae => Self::Vae,
            DomainModelAssetKind::TextEncoder => Self::TextEncoder,
            DomainModelAssetKind::Clip => Self::Clip,
            DomainModelAssetKind::ClipVision => Self::ClipVision,
            DomainModelAssetKind::Lora => Self::Lora,
            DomainModelAssetKind::Controlnet => Self::Controlnet,
            DomainModelAssetKind::Upscaler => Self::Upscaler,
            DomainModelAssetKind::Embedding => Self::Embedding,
            DomainModelAssetKind::Other => Self::Other,
        }
    }
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

impl From<DomainModelAssetSource> for ModelAssetSource {
    fn from(source: DomainModelAssetSource) -> Self {
        match source {
            DomainModelAssetSource::Huggingface {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAsset {
    pub id: String,
    pub name: String,
    pub model_asset_kind: ModelAssetKind,
    pub file_size_bytes: u64,
    pub download_source: ModelAssetSource,
    pub install: ModelAssetInstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAssetInstall {
    pub comfyui_relative_path: String,
}

impl From<&ModelAsset> for DomainModelAsset {
    fn from(asset: &ModelAsset) -> Self {
        Self {
            id: asset.id.clone(),
            name: asset.name.clone(),
            model_asset_kind: (&asset.model_asset_kind).into(),
            file_size_bytes: asset.file_size_bytes,
            download_source: (&asset.download_source).into(),
            install: crate::domain::workflow::ModelAssetInstall {
                comfyui_relative_path: asset.install.comfyui_relative_path.clone(),
            },
        }
    }
}

impl From<DomainModelAsset> for ModelAsset {
    fn from(asset: DomainModelAsset) -> Self {
        Self {
            id: asset.id,
            name: asset.name,
            model_asset_kind: asset.model_asset_kind.into(),
            file_size_bytes: asset.file_size_bytes,
            download_source: asset.download_source.into(),
            install: ModelAssetInstall {
                comfyui_relative_path: asset.install.comfyui_relative_path,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainCustomNodeGitSource> for CustomNodeGitSource {
    fn from(source: DomainCustomNodeGitSource) -> Self {
        match source {
            DomainCustomNodeGitSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomNodeInstall {
    pub comfyui_custom_nodes_relative_path: String,
    pub python_requirements_path: Option<String>,
}

impl From<&CustomNodeInstall> for DomainCustomNodeInstall {
    fn from(install: &CustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path.clone(),
            python_requirements_path: install.python_requirements_path.clone(),
        }
    }
}

impl From<DomainCustomNodeInstall> for CustomNodeInstall {
    fn from(install: DomainCustomNodeInstall) -> Self {
        Self {
            comfyui_custom_nodes_relative_path: install.comfyui_custom_nodes_relative_path,
            python_requirements_path: install.python_requirements_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainCustomNode> for CustomNode {
    fn from(node: DomainCustomNode) -> Self {
        Self {
            id: node.id,
            name: node.name,
            git_source: node.git_source.into(),
            install: node.install.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainWorkflowExecutionType> for WorkflowExecutionType {
    fn from(execution_type: DomainWorkflowExecutionType) -> Self {
        match execution_type {
            DomainWorkflowExecutionType::T2i => Self::T2i,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainComfyUiRuntimeSource> for ComfyUiRuntimeSource {
    fn from(source: DomainComfyUiRuntimeSource) -> Self {
        match source {
            DomainComfyUiRuntimeSource::Git {
                repository_url,
                revision,
            } => Self::Git {
                repository_url,
                revision,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainWorkflowPreset> for WorkflowPreset {
    fn from(preset: DomainWorkflowPreset) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCatalog {
    pub id: String,
    pub version: String,
    pub workflow_presets: Vec<WorkflowPreset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<crate::domain::profiles::ProvisioningComputeType> for ProvisioningComputeType {
    fn from(compute_type: crate::domain::profiles::ProvisioningComputeType) -> Self {
        match compute_type {
            crate::domain::profiles::ProvisioningComputeType::Pod => Self::Pod,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<crate::domain::profiles::ProvisioningStatusEndpoint> for ProvisioningStatusEndpoint {
    fn from(endpoint: crate::domain::profiles::ProvisioningStatusEndpoint) -> Self {
        Self {
            port: endpoint.port,
            protocol: endpoint.protocol,
            status_path: endpoint.status_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionerWorkerRuntime {
    pub provisioner_version: String,
    pub docker_image_ref: String,
    pub volume_mount_path: String,
    pub container_disk_bytes: u64,
    pub compute_type: ProvisioningComputeType,
    pub status_endpoint: ProvisioningStatusEndpoint,
}

impl From<&ProvisionerWorkerRuntime> for DomainProvisionerWorkerRuntime {
    fn from(runtime: &ProvisionerWorkerRuntime) -> Self {
        Self {
            provisioner_version: runtime.provisioner_version.clone(),
            docker_image_ref: runtime.docker_image_ref.clone(),
            volume_mount_path: runtime.volume_mount_path.clone(),
            container_disk_bytes: runtime.container_disk_bytes,
            compute_type: (&runtime.compute_type).into(),
            status_endpoint: (&runtime.status_endpoint).into(),
        }
    }
}

impl From<DomainProvisionerWorkerRuntime> for ProvisionerWorkerRuntime {
    fn from(runtime: DomainProvisionerWorkerRuntime) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointWorkerRuntime {
    pub endpoint_worker_version: String,
    pub docker_image_ref: String,
    pub http_port: u16,
    pub health_path: String,
    pub invoke_path: String,
}

impl From<&EndpointWorkerRuntime> for crate::domain::profiles::EndpointWorkerRuntime {
    fn from(runtime: &EndpointWorkerRuntime) -> Self {
        Self {
            endpoint_worker_version: runtime.endpoint_worker_version.clone(),
            docker_image_ref: runtime.docker_image_ref.clone(),
            http_port: runtime.http_port,
            health_path: runtime.health_path.clone(),
            invoke_path: runtime.invoke_path.clone(),
        }
    }
}

impl From<crate::domain::profiles::EndpointWorkerRuntime> for EndpointWorkerRuntime {
    fn from(runtime: crate::domain::profiles::EndpointWorkerRuntime) -> Self {
        Self {
            endpoint_worker_version: runtime.endpoint_worker_version,
            docker_image_ref: runtime.docker_image_ref,
            http_port: runtime.http_port,
            health_path: runtime.health_path,
            invoke_path: runtime.invoke_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainProvisioningProfile<RunPodProvisioningProfileConfig>> for ProvisioningProfile {
    fn from(profile: DomainProvisioningProfile<RunPodProvisioningProfileConfig>) -> Self {
        match profile.gpu_cloud_provider_id {
            DomainGpuCloudProviderId::Runpod => Self::Runpod {
                id: profile.id,
                version: profile.version,
                name: profile.name,
                provisioner_worker_runtime: profile.provisioner_worker_runtime.into(),
                gpu_cloud_provider_config: profile.gpu_cloud_provider_config,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl From<DomainEndpointProfile<RunPodEndpointProfileConfig>> for EndpointProfile {
    fn from(profile: DomainEndpointProfile<RunPodEndpointProfileConfig>) -> Self {
        match profile.gpu_cloud_provider_id {
            DomainGpuCloudProviderId::Runpod => Self::Runpod {
                id: profile.id,
                version: profile.version,
                name: profile.name,
                workflow_execution_type: profile.workflow_execution_type.into(),
                endpoint_worker_runtime: profile.endpoint_worker_runtime.into(),
                gpu_cloud_provider_config: profile.gpu_cloud_provider_config,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl
    From<
        DomainPlacementPlan<
            DomainProvisioningProfile<RunPodProvisioningProfileConfig>,
            DomainEndpointProfile<RunPodEndpointProfileConfig>,
        >,
    > for PlacementPlan
{
    fn from(
        plan: DomainPlacementPlan<
            DomainProvisioningProfile<RunPodProvisioningProfileConfig>,
            DomainEndpointProfile<RunPodEndpointProfileConfig>,
        >,
    ) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLifecycleState {
    Draft,
    Provisioning,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResourceStatus {
    Creating,
    Running,
    Ready,
    Terminated,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentStorageVolumeSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub provisioned_size_bytes: u64,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisioningPodSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub provisioning_profile_id: String,
    pub provisioner_status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerlessEndpointSnapshot {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub provider_resource_id: String,
    pub datacenter_id: String,
    pub provider_resource_status: ProviderResourceStatus,
    pub selected_gpu_id: String,
    pub endpoint_profile_id: String,
    pub endpoint_invoke_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCatalog {
    pub workspaces: Vec<Workspace>,
}

impl From<DomainWorkspaceLifecycleState> for WorkspaceLifecycleState {
    fn from(state: DomainWorkspaceLifecycleState) -> Self {
        match state {
            DomainWorkspaceLifecycleState::Draft => Self::Draft,
            DomainWorkspaceLifecycleState::Provisioning => Self::Provisioning,
            DomainWorkspaceLifecycleState::Ready => Self::Ready,
            DomainWorkspaceLifecycleState::Failed => Self::Failed,
        }
    }
}

impl From<DomainProviderResourceStatus> for ProviderResourceStatus {
    fn from(status: DomainProviderResourceStatus) -> Self {
        match status {
            DomainProviderResourceStatus::Creating => Self::Creating,
            DomainProviderResourceStatus::Running => Self::Running,
            DomainProviderResourceStatus::Ready => Self::Ready,
            DomainProviderResourceStatus::Terminated => Self::Terminated,
            DomainProviderResourceStatus::Failed => Self::Failed,
            DomainProviderResourceStatus::Unknown => Self::Unknown,
        }
    }
}

impl From<DomainPersistentStorageVolumeSnapshot> for PersistentStorageVolumeSnapshot {
    fn from(snapshot: DomainPersistentStorageVolumeSnapshot) -> Self {
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

impl From<DomainProvisioningPodSnapshot> for ProvisioningPodSnapshot {
    fn from(snapshot: DomainProvisioningPodSnapshot) -> Self {
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

impl From<DomainServerlessEndpointSnapshot> for ServerlessEndpointSnapshot {
    fn from(snapshot: DomainServerlessEndpointSnapshot) -> Self {
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

impl
    From<
        DomainWorkspace<
            DomainProvisioningProfile<RunPodProvisioningProfileConfig>,
            DomainEndpointProfile<RunPodEndpointProfileConfig>,
        >,
    > for Workspace
{
    fn from(
        workspace: DomainWorkspace<
            DomainProvisioningProfile<RunPodProvisioningProfileConfig>,
            DomainEndpointProfile<RunPodEndpointProfileConfig>,
        >,
    ) -> Self {
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

#[cfg(test)]
#[path = "workspace_contract_tests.rs"]
mod workspace_contract_tests;
