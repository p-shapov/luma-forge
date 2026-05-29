use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    domain::{
        placement as domain_placement, provider_inventory as domain_inventory,
        provider_setup as domain_provider_setup, provisioner as domain_provisioner,
        runtime as domain_runtime, workflow as domain_workflow, workspace as domain_workspace,
    },
    workspace_setup::contracts::{CreateWorkspaceInput, ProviderPlacementOptions},
};

// Command-boundary metadata only. These remote definitions provide generated
// binding shapes for domain types without making domain modules depend on Specta.
#[allow(dead_code)]
mod remote_types {
    use super::*;

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
        pub download_source: domain_workflow::ModelAssetSource,
        pub install_comfyui_relative_path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowExecutionType)]
    #[serde(rename_all = "snake_case")]
    pub(super) enum WorkflowExecutionType {
        T2i,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_runtime::ResolvedRuntimeImageSnapshot)]
    pub(super) struct ResolvedRuntimeImageSnapshot {
        pub contract_id: String,
        pub contract_version: String,
        pub endpoint_image_ref: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_provisioner::ResolvedProvisionerImageSnapshot)]
    pub(super) struct ResolvedProvisionerImageSnapshot {
        pub contract_id: String,
        pub contract_version: String,
        pub provisioner_worker_image_ref: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::RuntimeContractReference)]
    pub(super) struct RuntimeContractReference {
        pub id: String,
        pub version: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_provisioner::ProvisionerContractReference)]
    pub(super) struct ProvisionerContractReference {
        pub id: String,
        pub version: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowPreset)]
    pub(super) struct WorkflowPreset {
        pub id: String,
        pub version: String,
        pub name: String,
        pub workflow_execution_type: domain_workflow::WorkflowExecutionType,
        pub required_base_volume_size_bytes: u64,
        pub requires_hugging_face_api_key: bool,
        pub runtime_contract: domain_workflow::RuntimeContractReference,
        pub provisioner_contract: domain_provisioner::ProvisionerContractReference,
        pub required_model_assets: Vec<domain_workflow::ModelAsset>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workflow::WorkflowCatalog)]
    pub(super) struct WorkflowCatalog {
        pub workflow_presets: Vec<domain_workflow::WorkflowPreset>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_placement::PlacementPlan)]
    #[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
    pub(super) enum PlacementPlan {
        Runpod {
            selected_datacenter_id: String,
            selected_gpu_id: String,
            persistent_storage_volume_size_bytes: u64,
            endpoint_keep_alive_seconds: u32,
            selected_workflow_preset: domain_workflow::WorkflowPreset,
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
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ProvisioningPodSnapshot)]
    pub(super) struct ProvisioningPodSnapshot {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_resource_id: String,
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
        pub provisioner_status_url: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ServerlessEndpointSnapshot)]
    pub(super) struct ServerlessEndpointSnapshot {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub provider_resource_id: String,
        pub provider_resource_status: domain_workspace::ProviderResourceStatus,
        pub endpoint_invoke_url: String,
        pub provider_metadata: Option<domain_workspace::ServerlessEndpointProviderMetadata>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::ServerlessEndpointProviderMetadata)]
    #[serde(tag = "gpu_cloud_provider_id", rename_all = "snake_case")]
    pub(super) enum ServerlessEndpointProviderMetadata {
        Runpod { template_id: String },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::Workspace)]
    pub(super) struct Workspace {
        pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
        pub id: String,
        pub name: String,
        pub lifecycle_state: domain_workspace::WorkspaceLifecycleState,
        pub placement_plan: domain_placement::PlacementPlan,
        pub resolved_runtime_image: domain_runtime::ResolvedRuntimeImageSnapshot,
        pub resolved_provisioner_image: domain_provisioner::ResolvedProvisionerImageSnapshot,
        pub persistent_storage_volume_snapshot:
            Option<domain_workspace::PersistentStorageVolumeSnapshot>,
        pub active_provisioning_pod_snapshot: Option<domain_workspace::ProvisioningPodSnapshot>,
        pub serverless_endpoint_snapshot: Option<domain_workspace::ServerlessEndpointSnapshot>,
        pub last_provisioning_pod_snapshot: Option<domain_workspace::ProvisioningPodSnapshot>,
        pub environment_prepared_at: Option<String>,
        pub last_provisioning_failure: Option<domain_workspace::WorkspaceProvisioningFailure>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_workspace::WorkspaceCatalog)]
    pub(super) struct WorkspaceCatalog {
        pub workspaces: Vec<domain_workspace::Workspace>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_placement::ProviderPlacementCapabilities)]
    pub(super) struct ProviderPlacementCapabilities {
        pub endpoint_keep_alive: domain_placement::EndpointKeepAliveCapability,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[specta(remote = domain_placement::EndpointKeepAliveCapability)]
    #[serde(tag = "supported", rename_all = "snake_case")]
    pub(super) enum EndpointKeepAliveCapability {
        #[serde(rename = "true")]
        Supported {
            default_seconds: u32,
            min_seconds: u32,
            max_seconds: u32,
        },
        #[serde(rename = "false")]
        Unsupported,
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
pub struct GetProviderPlacementOptionsRequest {
    pub gpu_cloud_provider_id: domain_provider_setup::GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderPlacementOptionsResponse {
    pub provider_inventory: domain_inventory::ProviderInventory,
    pub placement_capabilities: domain_placement::ProviderPlacementCapabilities,
}

impl From<ProviderPlacementOptions> for GetProviderPlacementOptionsResponse {
    fn from(response: ProviderPlacementOptions) -> Self {
        Self {
            provider_inventory: response.provider_inventory,
            placement_capabilities: response.placement_capabilities,
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
