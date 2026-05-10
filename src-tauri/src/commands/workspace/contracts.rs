use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::{
        placement as domain_placement, profiles as domain_profiles,
        provider_inventory as domain_inventory, provider_setup as domain_provider_setup,
        workflow as domain_workflow, workspace as domain_workspace,
    },
    workspace_setup::contracts::CreateWorkspaceInput,
};

// Command-boundary metadata only. These remote definitions provide generated
// binding shapes for domain types without making domain modules depend on Specta.
#[allow(dead_code)]
mod remote_types {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::ModelAssetKind)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum ModelAssetKind {
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

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::ModelAssetSource)]
    #[serde(tag = "source_type", rename_all = "snake_case")]
    pub(super) enum ModelAssetSource {
        Huggingface {
            repository_id: String,
            file_path: String,
            revision: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::ModelAsset)]
    pub(super) struct ModelAsset {
        pub id: String,
        pub name: String,
        pub model_asset_kind: domain_workflow::ModelAssetKind,
        pub file_size_bytes: u64,
        pub download_source: domain_workflow::ModelAssetSource,
        pub install: domain_workflow::ModelAssetInstall,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::ModelAssetInstall)]
    pub(super) struct ModelAssetInstall {
        pub comfyui_relative_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::CustomNodeGitSource)]
    #[serde(tag = "source_type", rename_all = "snake_case")]
    pub(super) enum CustomNodeGitSource {
        Git {
            repository_url: String,
            revision: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::CustomNodeInstall)]
    pub(super) struct CustomNodeInstall {
        pub comfyui_custom_nodes_relative_path: String,
        pub python_requirements_path: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::CustomNode)]
    pub(super) struct CustomNode {
        pub id: String,
        pub name: String,
        pub git_source: domain_workflow::CustomNodeGitSource,
        pub install: domain_workflow::CustomNodeInstall,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowExecutionType)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkflowExecutionType {
        T2i,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::ComfyUiRuntimeSource)]
    #[serde(tag = "source_type", rename_all = "snake_case")]
    pub(super) enum ComfyUiRuntimeSource {
        Git {
            repository_url: String,
            revision: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowPreset)]
    pub(super) struct WorkflowPreset {
        pub id: String,
        pub version: String,
        pub name: String,
        pub workflow_execution_type: domain_workflow::WorkflowExecutionType,
        pub required_base_volume_size_bytes: u64,
        pub required_comfyui_source: domain_workflow::ComfyUiRuntimeSource,
        pub required_model_assets: Vec<domain_workflow::ModelAsset>,
        pub required_custom_nodes: Vec<domain_workflow::CustomNode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowCatalog)]
    pub(super) struct WorkflowCatalog {
        pub id: String,
        pub version: String,
        pub workflow_presets: Vec<domain_workflow::WorkflowPreset>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::ProvisioningComputeType)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum ProvisioningComputeType {
        Pod,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::ProvisioningStatusEndpoint)]
    pub(super) struct ProvisioningStatusEndpoint {
        pub port: u16,
        pub protocol: String,
        pub status_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::ProvisionerWorkerRuntime)]
    pub(super) struct ProvisionerWorkerRuntime {
        pub provisioner_version: String,
        pub docker_image_ref: String,
        pub volume_mount_path: String,
        pub container_disk_bytes: u64,
        pub compute_type: domain_profiles::ProvisioningComputeType,
        pub status_endpoint: domain_profiles::ProvisioningStatusEndpoint,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::EndpointWorkerRuntime)]
    pub(super) struct EndpointWorkerRuntime {
        pub endpoint_worker_version: String,
        pub docker_image_ref: String,
        pub http_port: u16,
        pub health_path: String,
        pub invoke_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::RunPodProvisioningProfileConfig)]
    pub(super) struct RunPodProvisioningProfileConfig {
        pub cloud_type: Option<String>,
        pub pod_template_id: Option<String>,
        pub network_volume_mount_path: String,
        pub expose_http_ports: Vec<u16>,
        pub env: Option<BTreeMap<String, String>>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::RunPodServerlessScalingConfig)]
    pub(super) struct RunPodServerlessScalingConfig {
        pub min_workers: u32,
        pub max_workers: u32,
        pub idle_timeout_seconds: u32,
        pub scaler_type: Option<String>,
        pub scaler_value: Option<u32>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::RunPodEndpointProfileConfig)]
    pub(super) struct RunPodEndpointProfileConfig {
        pub endpoint_template_id: Option<String>,
        pub container_disk_bytes: u64,
        pub volume_mount_path: String,
        pub env: Option<BTreeMap<String, String>>,
        pub scaling: domain_profiles::RunPodServerlessScalingConfig,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::ProvisioningProfile)]
    #[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
    pub(super) enum ProvisioningProfile {
        Runpod {
            id: String,
            version: String,
            name: String,
            provisioner_worker_runtime: domain_profiles::ProvisionerWorkerRuntime,
            gpu_cloud_provider_config: domain_profiles::RunPodProvisioningProfileConfig,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_profiles::EndpointProfile)]
    #[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
    pub(super) enum EndpointProfile {
        Runpod {
            id: String,
            version: String,
            name: String,
            workflow_execution_type: domain_workflow::WorkflowExecutionType,
            endpoint_worker_runtime: domain_profiles::EndpointWorkerRuntime,
            gpu_cloud_provider_config: domain_profiles::RunPodEndpointProfileConfig,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_placement::PlacementPlan)]
    #[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
    pub(super) enum PlacementPlan {
        Runpod {
            selected_datacenter_id: String,
            selected_gpu_id: String,
            persistent_storage_volume_size_bytes: u64,
            selected_workflow_preset: domain_workflow::WorkflowPreset,
            selected_provisioning_profile: domain_profiles::ProvisioningProfile,
            selected_endpoint_profile: domain_profiles::EndpointProfile,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceLifecycleState)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkspaceLifecycleState {
        Draft,
        Provisioning,
        Ready,
        Failed,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ProviderResourceStatus)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum ProviderResourceStatus {
        Creating,
        Running,
        Ready,
        Terminated,
        Failed,
        Unknown,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::PersistentStorageVolumeSnapshot)]
    pub(super) struct PersistentStorageVolumeSnapshot {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_resource_id: String,
        pub datacenter_id: String,
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
        pub provisioned_size_bytes: u64,
        pub mount_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ProvisioningPodSnapshot)]
    pub(super) struct ProvisioningPodSnapshot {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_resource_id: String,
        pub datacenter_id: String,
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
        pub selected_gpu_id: String,
        pub provisioning_profile_id: String,
        pub provisioner_status_url: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ServerlessEndpointSnapshot)]
    pub(super) struct ServerlessEndpointSnapshot {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_resource_id: String,
        pub datacenter_id: String,
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
        pub selected_gpu_id: String,
        pub endpoint_profile_id: String,
        pub endpoint_invoke_url: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::Workspace)]
    pub(super) struct Workspace {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub id: String,
        pub name: String,
        pub lifecycle_state: domain_workspace::WorkspaceLifecycleState,
        pub placement_plan: domain_placement::PlacementPlan,
        pub persistent_storage_volume_snapshot:
            Option<domain_workspace::PersistentStorageVolumeSnapshot>,
        pub active_provisioning_pod_snapshot: Option<domain_workspace::ProvisioningPodSnapshot>,
        pub serverless_endpoint_snapshot: Option<domain_workspace::ServerlessEndpointSnapshot>,
        pub last_provisioning_pod_snapshot: Option<domain_workspace::ProvisioningPodSnapshot>,
        pub environment_prepared_at: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceCatalog)]
    pub(super) struct WorkspaceCatalog {
        pub workspaces: Vec<domain_workspace::Workspace>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_inventory::GpuOption)]
    pub(super) struct GpuOption {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub id: String,
        pub name: String,
        pub vram_bytes: u64,
        pub availability_score: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_inventory::Datacenter)]
    pub(super) struct Datacenter {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub id: String,
        pub name: String,
        pub gpu_options: Vec<domain_inventory::GpuOption>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_inventory::ProviderInventory)]
    pub(super) struct ProviderInventory {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub fetched_at: String,
        pub max_persistent_storage_volume_size_bytes: Option<u64>,
        pub datacenters: Vec<domain_inventory::Datacenter>,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: domain_workflow::WorkflowCatalog,
}

impl From<domain_workflow::WorkflowCatalog> for GetWorkflowCatalogResponse {
    fn from(response: domain_workflow::WorkflowCatalog) -> Self {
        Self {
            workflow_catalog: response,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<domain_profiles::ProvisioningProfile>,
}

impl From<Vec<domain_profiles::ProvisioningProfile>> for GetProvisioningProfilesResponse {
    fn from(response: Vec<domain_profiles::ProvisioningProfile>) -> Self {
        Self {
            provisioning_profiles: response,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<domain_profiles::EndpointProfile>,
}

impl From<Vec<domain_profiles::EndpointProfile>> for GetEndpointProfilesResponse {
    fn from(response: Vec<domain_profiles::EndpointProfile>) -> Self {
        Self {
            endpoint_profiles: response,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryResponse {
    pub provider_inventory: domain_inventory::ProviderInventory,
}

impl From<domain_inventory::ProviderInventory> for GetProviderInventoryResponse {
    fn from(response: domain_inventory::ProviderInventory) -> Self {
        Self {
            provider_inventory: response,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: domain_workspace::WorkspaceCatalog,
}

impl From<domain_workspace::WorkspaceCatalog> for GetWorkspaceCatalogResponse {
    fn from(response: domain_workspace::WorkspaceCatalog) -> Self {
        Self {
            workspace_catalog: response,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
    pub placement_plan: domain_placement::PlacementPlan,
}

impl From<CreateWorkspaceRequest> for CreateWorkspaceInput {
    fn from(request: CreateWorkspaceRequest) -> Self {
        Self {
            workspace_id: request.workspace_id,
            name: request.name,
            gpu_cloud_provider_id: request.gpu_cloud_provider_id,
            placement_plan: request.placement_plan,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceResponse {
    pub workspace: domain_workspace::Workspace,
}

impl From<domain_workspace::Workspace> for CreateWorkspaceResponse {
    fn from(response: domain_workspace::Workspace) -> Self {
        Self {
            workspace: response,
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
