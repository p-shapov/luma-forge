use std::{future::Future, pin::Pin};

pub mod contracts;
pub mod error;

use crate::{
    domain::{
        placement::validator as placement_validator,
        provider_inventory::validator as provider_inventory_validator,
        provider_setup::GpuCloudProviderId,
        workflow::WorkflowCatalog,
        workspace::{Workspace, WorkspaceCatalog},
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use contracts::{CreateWorkspaceInput, ProviderPlacementOptions};
use error::WorkspaceSetupError;

pub trait ProviderPlacementOptionsGateway: Send + Sync {
    fn fetch_placement_options<'a>(
        &'a self,
        provider_id: &'a GpuCloudProviderId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ProviderPlacementOptions, WorkspaceSetupError>> + Send + 'a>,
    >;
}

pub trait WorkspaceSetupCatalogReader: Send + Sync {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError>;
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
    P: ProviderPlacementOptionsGateway,
    W: WorkspaceCatalogRepository,
{
    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        self.catalogs.workflow_catalog()
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
        let options = self.providers.fetch_placement_options(&provider_id).await?;
        provider_inventory_validator::validate_provider_inventory(
            provider_id,
            &options.provider_inventory,
        )
        .map_err(|_| WorkspaceSetupError::ProviderInventoryInvalid)?;
        Ok(options)
    }

    pub async fn get_workspace_catalog(&self) -> Result<WorkspaceCatalog, WorkspaceSetupError> {
        self.workspace_catalog.list_workspaces().await
    }

    pub async fn create_workspace(
        &self,
        request: CreateWorkspaceInput,
    ) -> Result<Workspace, WorkspaceSetupError> {
        let workspace_id = uuid::Uuid::parse_str(&request.workspace_id)
            .map_err(|_| WorkspaceSetupError::InvalidWorkspaceId)?;
        let name = request.name.trim();
        if name.is_empty() {
            return Err(WorkspaceSetupError::WorkspaceNameRequired);
        }

        let provider_id = request.gpu_cloud_provider_id;
        self.secrets
            .read_api_key(&provider_id)?
            .ok_or(WorkspaceSetupError::ProviderSetupIncomplete)?;

        let workflow_catalog = self.catalogs.workflow_catalog()?;
        placement_validator::validate_placement_plan(
            provider_id,
            &request.placement_plan,
            &workflow_catalog,
        )
        .map_err(WorkspaceSetupError::from)?;

        let workspace = Workspace::new_draft(
            provider_id,
            workspace_id.to_string(),
            name.to_string(),
            request.placement_plan,
        )
        .map_err(|_| WorkspaceSetupError::InvalidWorkspaceMetadata)?;

        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        Ok(workspace)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;
