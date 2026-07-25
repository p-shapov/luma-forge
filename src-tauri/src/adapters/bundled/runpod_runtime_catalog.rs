use crate::{
    application::runtimes::{
        runpod::{
            RunpodContractRequirements, RunpodRuntimeCatalog, RunpodRuntimeCatalogError,
            RunpodRuntimeDefinition,
        },
        CatalogRef,
    },
    infra::bundled::{
        entries::{runtime_contracts, runtime_presets},
        errors::BundledCatalogError,
    },
};

use super::BundledCatalogAdapter;

#[luma_diagnostics::diagnostic]
#[async_trait::async_trait]
impl RunpodRuntimeCatalog for BundledCatalogAdapter {
    #[diagnostic(show_output, show_error)]
    async fn resolve(
        &self,
        #[diagnostic(show)] preset: &CatalogRef,
        #[diagnostic(show)] requirements: &RunpodContractRequirements,
    ) -> Result<RunpodRuntimeDefinition, RunpodRuntimeCatalogError> {
        runtime_presets::Entry::get(&self.catalog, (&preset.id, &preset.revision))
            .await
            .map_err(map_catalog_error)?
            .ok_or(RunpodRuntimeCatalogError::InvalidCatalog)?;
        let provisioner = runtime_contracts::Entry::get(
            &self.catalog,
            (
                &requirements.provisioner_contract_ref.id,
                &requirements.provisioner_contract_ref.revision,
            ),
        )
        .await
        .map_err(map_catalog_error)?
        .ok_or(RunpodRuntimeCatalogError::InvalidCatalog)?;
        let endpoint = runtime_contracts::Entry::get(
            &self.catalog,
            (
                &requirements.endpoint_contract_ref.id,
                &requirements.endpoint_contract_ref.revision,
            ),
        )
        .await
        .map_err(map_catalog_error)?
        .ok_or(RunpodRuntimeCatalogError::InvalidCatalog)?;

        Ok(RunpodRuntimeDefinition {
            provisioner_image_ref: String::from(provisioner.runtime_contract.image_ref),
            endpoint_image_ref: String::from(endpoint.runtime_contract.image_ref),
        })
    }
}

fn map_catalog_error(error: BundledCatalogError) -> RunpodRuntimeCatalogError {
    match error {
        BundledCatalogError::Io { .. } => RunpodRuntimeCatalogError::Unavailable,
        BundledCatalogError::Json { .. }
        | BundledCatalogError::Contract { .. }
        | BundledCatalogError::Entry { .. } => RunpodRuntimeCatalogError::InvalidCatalog,
    }
}
