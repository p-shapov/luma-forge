use crate::{
    bundled_catalog::reader::BundledCatalogReader,
    workspace_catalog::migrations::WorkspaceCatalogMigrationSource,
    workspace_setup::WorkspaceSetupCatalogReader,
};

pub(crate) fn test_migration_source() -> WorkspaceCatalogMigrationSource {
    let reader = BundledCatalogReader;
    WorkspaceCatalogMigrationSource::new(
        reader.workflow_catalog().expect("workflow catalog"),
        reader
            .provisioning_profiles()
            .expect("provisioning profiles"),
        reader.endpoint_profiles().expect("endpoint profiles"),
    )
}
