use crate::domain::runtime_contract::RuntimeCatalog;

use super::RuntimeCatalogError;

const RUNTIME_CONTRACTS_JSON: &str = include_str!("../../../bundled/runtime-contracts.json");

#[derive(Debug, Clone, Copy, Default)]
pub struct BundledRuntimeContractCatalogReader;

impl BundledRuntimeContractCatalogReader {
    pub fn read_runtime_contract_catalog(&self) -> Result<RuntimeCatalog, RuntimeCatalogError> {
        serde_json::from_str(RUNTIME_CONTRACTS_JSON).map_err(parse_error)
    }
}

fn parse_error(error: serde_json::Error) -> RuntimeCatalogError {
    RuntimeCatalogError::ParseFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::BundledRuntimeContractCatalogReader;

    #[test]
    fn bundled_runtime_contract_reader_deserializes_contracts() {
        let catalog = BundledRuntimeContractCatalogReader
            .read_runtime_contract_catalog()
            .expect("bundled runtime contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "comfyui-py312-cu126-torch291"),
            "expected bundled ComfyUI endpoint contract"
        );
        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "luma-forge-provisioner"),
            "expected bundled provisioner contract"
        );
    }
}
