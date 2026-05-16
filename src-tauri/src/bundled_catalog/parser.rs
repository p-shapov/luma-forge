use crate::{
    bundled_catalog::error::BundledCatalogError,
    domain::{
        runtime::{validator::validate_runtime_catalog, RuntimeCatalog},
        workflow::validator::validate_workflow_catalog,
        workflow::WorkflowCatalog,
    },
};

pub(super) fn parse_runtime_catalog(value: &str) -> Result<RuntimeCatalog, BundledCatalogError> {
    let catalog: RuntimeCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_runtime_catalog(&catalog).map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}

pub(super) fn parse_workflow_catalog(
    value: &str,
    runtime_catalog: &RuntimeCatalog,
) -> Result<WorkflowCatalog, BundledCatalogError> {
    let catalog: WorkflowCatalog =
        serde_json::from_str(value).map_err(|_| BundledCatalogError::ParseFailed)?;
    validate_workflow_catalog(&catalog, runtime_catalog)
        .map_err(|_| BundledCatalogError::ValidationFailed)?;
    Ok(catalog)
}
