use crate::domain::runtime_contract::RuntimeCatalog;

use super::{
    reader::BundledRuntimeContractCatalogReader, validation::validate_runtime_catalog,
    RuntimeCatalogError,
};

#[derive(Debug, Clone, Default)]
pub struct RuntimeCatalogService {
    reader: BundledRuntimeContractCatalogReader,
}

impl RuntimeCatalogService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_runtime_contract_catalog(&self) -> Result<RuntimeCatalog, RuntimeCatalogError> {
        let catalog = self.reader.read_runtime_contract_catalog()?;

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
            .any(|contract| contract.id == "comfyui-py312-cu126-torch291"));
        assert!(catalog
            .contracts
            .iter()
            .any(|contract| contract.id == "luma-forge-provisioner"));
    }
}
