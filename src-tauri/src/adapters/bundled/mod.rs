mod runpod_runtime_catalog;
mod workflow_catalog;

use crate::infra::bundled::Catalog;

#[derive(Debug)]
pub struct BundledCatalogAdapter {
    catalog: Catalog,
}

impl BundledCatalogAdapter {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }
}
