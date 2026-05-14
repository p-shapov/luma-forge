use crate::{
    bundled_catalog::error::BundledCatalogError,
    domain::{workflow::validator::validate_workflow_catalog, workflow::WorkflowCatalog},
};

pub(super) fn parse_workflow_catalog(value: &str) -> Result<WorkflowCatalog, BundledCatalogError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_workflow_catalog(&catalog).map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}
