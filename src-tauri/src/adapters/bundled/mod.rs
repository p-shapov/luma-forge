mod runpod_runtime_catalog;
mod workflow_catalog;

use std::path::PathBuf;

use crate::infra::bundled::Catalog;

#[derive(Debug)]
pub struct BundledCatalogAdapter {
    catalog: Catalog,
}

impl BundledCatalogAdapter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            catalog: Catalog::new(root),
        }
    }
}
