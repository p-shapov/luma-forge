use std::{future::Future, pin::Pin};

use crate::{
    domain::{
        placement::validator as placement_validator,
        profiles::{EndpointProfile, ProvisioningProfile},
        provider_inventory::validator as provider_inventory_validator,
        provider_inventory::ProviderInventory,
        provider_setup::GpuCloudProviderId,
        workflow::WorkflowCatalog,
        workspace::{Workspace, WorkspaceCatalog},
    },
    secrets::SecretStore,
    workspace::{
        workspace_catalog_repository::WorkspaceCatalogRepository,
        workspace_setup_contracts::CreateWorkspaceInput,
        workspace_setup_error::WorkspaceSetupError,
    },
};

pub trait ProviderInventoryGateway: Send + Sync {
    fn fetch_inventory<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> Pin<Box<dyn Future<Output = Result<ProviderInventory, WorkspaceSetupError>> + Send + 'a>>;
}

pub trait WorkspaceSetupCatalogReader: Send + Sync {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError>;
    fn provisioning_profiles(&self) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError>;
    fn endpoint_profiles(&self) -> Result<Vec<EndpointProfile>, WorkspaceSetupError>;
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
    C: WorkspaceSetupCatalogReader,
    S: SecretStore,
    P: ProviderInventoryGateway,
    W: WorkspaceCatalogRepository,
{
    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        self.catalogs.workflow_catalog()
    }

    pub fn get_provisioning_profiles(
        &self,
    ) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError> {
        self.catalogs.provisioning_profiles()
    }

    pub fn get_endpoint_profiles(&self) -> Result<Vec<EndpointProfile>, WorkspaceSetupError> {
        self.catalogs.endpoint_profiles()
    }

    pub async fn get_provider_inventory(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ProviderInventory, WorkspaceSetupError> {
        let provider_inventory = self.providers.fetch_inventory(&provider_id).await?;
        provider_inventory_validator::validate_provider_inventory(provider_id, &provider_inventory)
            .map_err(|_| WorkspaceSetupError::ProviderApiUnavailable)?;
        Ok(provider_inventory)
    }

    pub async fn get_workspace_catalog(&self) -> Result<WorkspaceCatalog, WorkspaceSetupError> {
        self.workspace_catalog.list_workspaces().await
    }

    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceInput,
    ) -> Result<Workspace, WorkspaceSetupError> {
        let workspace_id = uuid::Uuid::parse_str(&request.workspace_id)
            .map_err(|_| WorkspaceSetupError::InvalidRequest)?;
        let name = request.name.trim();
        if name.is_empty() {
            return Err(WorkspaceSetupError::InvalidRequest);
        }

        let provider_id = request.gpu_cloud_provider_id;
        self.secrets
            .read_api_key(&provider_id)?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

        let workflow_catalog = self.catalogs.workflow_catalog()?;
        let provisioning_profiles = self.catalogs.provisioning_profiles()?;
        let endpoint_profiles = self.catalogs.endpoint_profiles()?;
        placement_validator::validate_placement_plan(
            provider_id,
            &request.placement_plan,
            &workflow_catalog,
            &provisioning_profiles,
            &endpoint_profiles,
        )
        .map_err(|_| WorkspaceSetupError::InvalidPlacementPlan)?;

        let workspace = Workspace::new_draft(
            provider_id,
            workspace_id.to_string(),
            name.to_string(),
            request.placement_plan,
        )
        .map_err(|_| WorkspaceSetupError::InvalidRequest)?;

        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        Ok(workspace)
    }
}

#[cfg(test)]
#[path = "workspace_setup_tests.rs"]
pub(crate) mod workspace_setup_tests;
