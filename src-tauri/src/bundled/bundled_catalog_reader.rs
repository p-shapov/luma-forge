use crate::{
    bundled::bundled_catalog_parser::{
        parse_endpoint_profiles, parse_provisioning_profiles, parse_workflow_catalog,
    },
    domain::{
        profiles::{EndpointProfile, ProvisioningProfile},
        workflow::WorkflowCatalog,
    },
    workspace_setup::{
        workspace_setup_error::WorkspaceSetupError,
        workspace_setup_service::WorkspaceSetupCatalogReader,
    },
};

const WORKFLOW_CATALOG_JSON: &str =
    include_str!("../../../resources/catalog/workflow-catalog.json");
const PROVISIONING_PROFILES_JSON: &str =
    include_str!("../../../resources/catalog/provisioning-profiles.json");
const ENDPOINT_PROFILES_JSON: &str =
    include_str!("../../../resources/catalog/endpoint-profiles.json");

#[derive(Debug, Clone, Default)]
pub struct BundledCatalogReader;

impl WorkspaceSetupCatalogReader for BundledCatalogReader {
    fn workflow_catalog(&self) -> Result<WorkflowCatalog, WorkspaceSetupError> {
        parse_workflow_catalog(WORKFLOW_CATALOG_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }

    fn provisioning_profiles(&self) -> Result<Vec<ProvisioningProfile>, WorkspaceSetupError> {
        parse_provisioning_profiles(PROVISIONING_PROFILES_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }

    fn endpoint_profiles(&self) -> Result<Vec<EndpointProfile>, WorkspaceSetupError> {
        parse_endpoint_profiles(ENDPOINT_PROFILES_JSON)
            .map_err(|_| WorkspaceSetupError::WorkflowCatalogUnavailable)
    }
}
