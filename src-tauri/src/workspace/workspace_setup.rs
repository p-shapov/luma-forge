use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

use crate::{
    bundled::{
        bundled_catalog::CatalogReader,
        bundled_contracts::{EndpointProfile, ProvisioningProfile},
    },
    domain::{
        provider_inventory::ProviderInventory, provider_setup::GpuCloudProviderId,
        workflow::WorkflowCatalog, workspace::WorkspaceLifecycleState,
    },
    provider_setup::{NativeCommandError, NativeCommandErrorCode, ProviderSetupError},
    secrets::SecretStore,
    workspace::{
        workspace_catalog::WorkspaceCatalogRepository,
        workspace_contracts::{PlacementPlan, Workspace, WorkspaceCatalog},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkflowCatalogResponse {
    pub workflow_catalog: WorkflowCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProvisioningProfilesResponse {
    pub provisioning_profiles: Vec<ProvisioningProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetEndpointProfilesResponse {
    pub endpoint_profiles: Vec<EndpointProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryRequest {
    pub gpu_cloud_provider_id: GpuCloudProviderId,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetProviderInventoryResponse {
    pub provider_inventory: ProviderInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GetWorkspaceCatalogResponse {
    pub workspace_catalog: WorkspaceCatalog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceRequest {
    pub workspace_id: String,
    pub name: String,
    pub gpu_cloud_provider_id: GpuCloudProviderId,
    pub placement_plan: PlacementPlan,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateWorkspaceResponse {
    pub workspace: Workspace,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WorkspaceSetupError {
    #[error("provider setup is incomplete")]
    ProviderSetupIncomplete,
    #[error("provider api unavailable")]
    ProviderApiUnavailable,
    #[error("secure keyring unavailable")]
    SecureKeyringUnavailable,
    #[error("workflow catalog unavailable")]
    WorkflowCatalogUnavailable,
    #[error("workspace catalog unavailable")]
    WorkspaceCatalogUnavailable,
    #[error("local storage unavailable")]
    LocalStorageUnavailable,
    #[error("invalid placement plan")]
    InvalidPlacementPlan,
    #[error("workspace already exists")]
    WorkspaceAlreadyExists,
    #[error("invalid request")]
    InvalidRequest,
}

impl WorkspaceSetupError {
    pub fn code(&self) -> NativeCommandErrorCode {
        match self {
            Self::ProviderSetupIncomplete => NativeCommandErrorCode::ProviderSetupIncomplete,
            Self::ProviderApiUnavailable => NativeCommandErrorCode::ProviderApiUnavailable,
            Self::SecureKeyringUnavailable => NativeCommandErrorCode::SecureKeyringUnavailable,
            Self::WorkflowCatalogUnavailable => NativeCommandErrorCode::WorkflowCatalogUnavailable,
            Self::WorkspaceCatalogUnavailable => {
                NativeCommandErrorCode::WorkspaceCatalogUnavailable
            }
            Self::LocalStorageUnavailable => NativeCommandErrorCode::LocalStorageUnavailable,
            Self::InvalidPlacementPlan => NativeCommandErrorCode::InvalidPlacementPlan,
            Self::WorkspaceAlreadyExists => NativeCommandErrorCode::WorkspaceAlreadyExists,
            Self::InvalidRequest => NativeCommandErrorCode::InvalidRequest,
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ProviderApiUnavailable
                | Self::SecureKeyringUnavailable
                | Self::WorkspaceCatalogUnavailable
                | Self::LocalStorageUnavailable
        )
    }

    pub fn ui_message(&self) -> &'static str {
        match self {
            Self::ProviderSetupIncomplete => "GPU cloud provider setup is incomplete.",
            Self::ProviderApiUnavailable => "Provider API is unavailable.",
            Self::SecureKeyringUnavailable => "Secure keyring is unavailable.",
            Self::WorkflowCatalogUnavailable => "Workflow catalog is unavailable.",
            Self::WorkspaceCatalogUnavailable => "Workspace catalog is unavailable.",
            Self::LocalStorageUnavailable => "Local storage is unavailable.",
            Self::InvalidPlacementPlan => "Placement plan is invalid.",
            Self::WorkspaceAlreadyExists => "Workspace already exists.",
            Self::InvalidRequest => "Request is invalid.",
        }
    }
}

impl From<WorkspaceSetupError> for NativeCommandError {
    fn from(error: WorkspaceSetupError) -> Self {
        Self {
            code: error.code(),
            message: error.ui_message().to_string(),
            retryable: error.retryable(),
        }
    }
}

impl From<ProviderSetupError> for WorkspaceSetupError {
    fn from(error: ProviderSetupError) -> Self {
        match error {
            ProviderSetupError::ProviderSetupIncomplete => Self::ProviderSetupIncomplete,
            ProviderSetupError::ProviderApiUnavailable
            | ProviderSetupError::InvalidProviderApiKey
            | ProviderSetupError::ProviderIdentityUnavailable => Self::ProviderApiUnavailable,
            ProviderSetupError::SecureKeyringUnavailable => Self::SecureKeyringUnavailable,
            ProviderSetupError::ProviderSetupAlreadyExists => Self::InvalidRequest,
        }
    }
}

pub trait ProviderInventoryGateway: Send + Sync {
    fn fetch_inventory<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
        api_key: &'a crate::domain::provider_setup::ProviderApiKey,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInventory, WorkspaceSetupError>> + Send + 'a>>;
}

pub struct WorkspaceSetupService<C, S, P, W> {
    catalogs: C,
    secrets: S,
    providers: P,
    workspace_catalog: W,
}

impl<C, S, P, W> WorkspaceSetupService<C, S, P, W> {
    pub fn new(catalogs: C, secrets: S, providers: P, workspace_catalog: W) -> Self {
        Self {
            catalogs,
            secrets,
            providers,
            workspace_catalog,
        }
    }
}

impl<C, S, P, W> WorkspaceSetupService<C, S, P, W>
where
    C: CatalogReader,
    S: SecretStore,
    P: ProviderInventoryGateway,
    W: WorkspaceCatalogRepository,
{
    pub fn get_workflow_catalog(&self) -> Result<GetWorkflowCatalogResponse, WorkspaceSetupError> {
        Ok(GetWorkflowCatalogResponse {
            workflow_catalog: self.catalogs.workflow_catalog()?,
        })
    }

    pub fn get_provisioning_profiles(
        &self,
    ) -> Result<GetProvisioningProfilesResponse, WorkspaceSetupError> {
        Ok(GetProvisioningProfilesResponse {
            provisioning_profiles: self.catalogs.provisioning_profiles()?,
        })
    }

    pub fn get_endpoint_profiles(
        &self,
    ) -> Result<GetEndpointProfilesResponse, WorkspaceSetupError> {
        Ok(GetEndpointProfilesResponse {
            endpoint_profiles: self.catalogs.endpoint_profiles()?,
        })
    }

    pub async fn get_provider_inventory(
        &self,
        request: GetProviderInventoryRequest,
    ) -> Result<GetProviderInventoryResponse, WorkspaceSetupError> {
        let provider_id = request.gpu_cloud_provider_id;
        let api_key = self
            .secrets
            .read_api_key(&provider_id)?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;
        let provider_inventory = self
            .providers
            .fetch_inventory(&provider_id, &api_key)
            .await?;

        Ok(GetProviderInventoryResponse { provider_inventory })
    }

    pub async fn get_workspace_catalog(
        &self,
    ) -> Result<GetWorkspaceCatalogResponse, WorkspaceSetupError> {
        Ok(GetWorkspaceCatalogResponse {
            workspace_catalog: self.workspace_catalog.list_workspaces().await?,
        })
    }

    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceRequest,
    ) -> Result<CreateWorkspaceResponse, WorkspaceSetupError> {
        let workspace_id = uuid::Uuid::parse_str(&request.workspace_id)
            .map_err(|_| WorkspaceSetupError::InvalidRequest)?;
        let name = request.name.trim();
        if name.is_empty() {
            return Err(WorkspaceSetupError::InvalidRequest);
        }

        self.secrets
            .read_api_key(&request.gpu_cloud_provider_id)?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

        let workflow_catalog = self.catalogs.workflow_catalog()?;
        let provisioning_profiles = self.catalogs.provisioning_profiles()?;
        let endpoint_profiles = self.catalogs.endpoint_profiles()?;
        validate_placement_plan(
            request.gpu_cloud_provider_id,
            &request.placement_plan,
            &workflow_catalog,
            &provisioning_profiles,
            &endpoint_profiles,
        )?;

        let workspace = Workspace {
            gpu_cloud_provider_id: request.gpu_cloud_provider_id,
            id: workspace_id.to_string(),
            name: name.to_string(),
            lifecycle_state: WorkspaceLifecycleState::Draft,
            placement_plan: request.placement_plan,
            persistent_storage_volume_snapshot: None,
            active_provisioning_pod_snapshot: None,
            serverless_endpoint_snapshot: None,
            last_provisioning_pod_snapshot: None,
            environment_prepared_at: None,
        };

        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        Ok(CreateWorkspaceResponse { workspace })
    }
}

fn validate_placement_plan(
    provider_id: GpuCloudProviderId,
    placement_plan: &PlacementPlan,
    workflow_catalog: &WorkflowCatalog,
    provisioning_profiles: &[ProvisioningProfile],
    endpoint_profiles: &[EndpointProfile],
) -> Result<(), WorkspaceSetupError> {
    if placement_plan.selected_datacenter_id.trim().is_empty()
        || placement_plan.selected_gpu_id.trim().is_empty()
    {
        return Err(WorkspaceSetupError::InvalidPlacementPlan);
    }
    let domain_placement_plan = placement_plan.to_domain();

    let preset = workflow_catalog
        .workflow_presets
        .iter()
        .find(|preset| preset.id == placement_plan.selected_workflow_preset.id)
        .ok_or(WorkspaceSetupError::InvalidPlacementPlan)?;
    if preset != &placement_plan.selected_workflow_preset {
        return Err(WorkspaceSetupError::InvalidPlacementPlan);
    }

    let provisioning_profile = provisioning_profiles
        .iter()
        .find(|profile| profile.id() == placement_plan.selected_provisioning_profile.id())
        .ok_or(WorkspaceSetupError::InvalidPlacementPlan)?;
    let provisioning_profile_core = &domain_placement_plan.selected_provisioning_profile;
    if provisioning_profile != &placement_plan.selected_provisioning_profile
        || provisioning_profile_core.gpu_cloud_provider_id != provider_id
    {
        return Err(WorkspaceSetupError::InvalidPlacementPlan);
    }

    let endpoint_profile = endpoint_profiles
        .iter()
        .find(|profile| profile.id() == placement_plan.selected_endpoint_profile.id())
        .ok_or(WorkspaceSetupError::InvalidPlacementPlan)?;
    let endpoint_profile_core = &domain_placement_plan.selected_endpoint_profile;
    if endpoint_profile != &placement_plan.selected_endpoint_profile
        || endpoint_profile_core.gpu_cloud_provider_id != provider_id
        || endpoint_profile_core.workflow_execution_type != preset.workflow_execution_type
    {
        return Err(WorkspaceSetupError::InvalidPlacementPlan);
    }

    if placement_plan.persistent_storage_volume_size_bytes < preset.required_base_volume_size_bytes
    {
        return Err(WorkspaceSetupError::InvalidPlacementPlan);
    }

    Ok(())
}

#[cfg(test)]
#[path = "workspace_setup_tests.rs"]
pub(crate) mod workspace_setup_tests;
