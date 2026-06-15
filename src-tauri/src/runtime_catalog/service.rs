use crate::domain::runtime_contract::RuntimeCatalog;

use super::{
    reader::read_bundled_runtime_contract_catalog, validation::validate_runtime_catalog,
    RuntimeCatalogError,
};

#[derive(Debug, Clone, Default)]
pub struct RuntimeCatalogService;

impl RuntimeCatalogService {
    pub fn new() -> Self {
        Self
    }

    pub fn get_runtime_contract_catalog(&self) -> Result<RuntimeCatalog, RuntimeCatalogError> {
        let catalog = read_bundled_runtime_contract_catalog()?;

        validate_runtime_catalog(&catalog)?;

        Ok(catalog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_runtime_contract_catalog_returns_valid_catalog() {
        let catalog = RuntimeCatalogService::new()
            .get_runtime_contract_catalog()
            .expect("runtime contract catalog should be valid");

        assert!(catalog
            .contracts
            .iter()
            .any(|contract| contract.id == "runpod-endpoint-comfyui-hidream-o1-dev"));
        assert!(catalog
            .contracts
            .iter()
            .any(|contract| contract.id == "provisioner"));
    }
}
