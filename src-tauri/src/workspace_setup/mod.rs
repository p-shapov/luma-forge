pub mod contracts;
pub mod error;
mod providers;

use crate::{
    domain::{
        placement::validator as placement_validator,
        provider_inventory::validator as provider_inventory_validator,
        provider_setup::GpuCloudProviderId,
        runtime::RuntimeCatalog,
        workflow::WorkflowCatalog,
        workspace::{Workspace, WorkspaceCatalog},
    },
    secrets::SecretStore,
    workspace_catalog::repository::WorkspaceCatalogRepository,
};

use contracts::{CreateWorkspaceInput, ProviderPlacementOptions};
use error::WorkspaceSetupError;

pub trait WorkspaceSetupCatalogReader: Send + Sync {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError>;
    fn runtime_catalog(&self) -> Result<RuntimeCatalog, WorkspaceSetupError>;
}

pub struct WorkspaceSetupService<C, S, W> {
    catalogs: C,
    secrets: S,
    workspace_catalog: W,
}

impl<C, S, W> WorkspaceSetupService<C, S, W> {
    pub fn new(catalogs: C, secrets: S, workspace_catalog: W) -> Self {
        Self {
            catalogs,
            secrets,
            workspace_catalog,
        }
    }
}

impl<C, S, W> WorkspaceSetupService<C, S, W>
where
    C: WorkspaceSetupCatalogReader,
    S: SecretStore,
    W: WorkspaceCatalogRepository,
{
    pub fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        self.catalogs.workflow_catalog()
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
        let options = providers::fetch_placement_options(&self.secrets, &provider_id).await?;
        self.validate_provider_placement_options(provider_id, options)
    }

    fn validate_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
        options: ProviderPlacementOptions,
    ) -> Result<ProviderPlacementOptions, WorkspaceSetupError> {
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
        let runtime_catalog = self.catalogs.runtime_catalog()?;
        placement_validator::validate_placement_plan(
            provider_id,
            &request.placement_plan,
            &workflow_catalog,
            &runtime_catalog,
        )
        .map_err(WorkspaceSetupError::from)?;
        let resolved_runtime_image = runtime_catalog
            .resolve(
                &request
                    .placement_plan
                    .selected_workflow_preset()
                    .runtime_contract
                    .id,
                &request
                    .placement_plan
                    .selected_workflow_preset()
                    .runtime_contract
                    .version,
            )
            .ok_or(WorkspaceSetupError::WorkflowCatalogUnavailable)?;

        let workspace = Workspace::new_draft(
            provider_id,
            workspace_id.to_string(),
            name.to_string(),
            request.placement_plan,
            resolved_runtime_image,
        )
        .map_err(|_| WorkspaceSetupError::InvalidWorkspaceMetadata)?;

        let workspace = self.workspace_catalog.insert_workspace(&workspace).await?;
        Ok(workspace)
    }
}
