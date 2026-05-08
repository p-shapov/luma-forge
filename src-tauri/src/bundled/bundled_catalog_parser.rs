use crate::{
    bundled::{
        bundled_catalog_error::BundledCatalogError,
        bundled_catalog_validator::{
            validate_endpoint_profiles, validate_provisioning_profiles, validate_workflow_catalog,
        },
    },
    workspace::workspace_contracts::{EndpointProfile, ProvisioningProfile, WorkflowCatalog},
};

pub(super) fn parse_workflow_catalog(value: &str) -> Result<WorkflowCatalog, BundledCatalogError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_workflow_catalog(&catalog)?;
    Ok(catalog)
}

pub(super) fn parse_provisioning_profiles(
    value: &str,
) -> Result<Vec<ProvisioningProfile>, BundledCatalogError> {
    let profiles: Vec<ProvisioningProfile> =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_provisioning_profiles(&profiles)?;
    Ok(profiles)
}

pub(super) fn parse_endpoint_profiles(
    value: &str,
) -> Result<Vec<EndpointProfile>, BundledCatalogError> {
    let profiles: Vec<EndpointProfile> =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_endpoint_profiles(&profiles)?;
    Ok(profiles)
}
