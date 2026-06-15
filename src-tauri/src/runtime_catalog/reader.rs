use crate::domain::runtime_contract::RuntimeCatalog;

use super::RuntimeCatalogError;

const RUNTIME_CONTRACTS_JSON: &str = include_str!("../../../bundled/runtime-contracts.json");

pub(super) fn read_bundled_runtime_contract_catalog() -> Result<RuntimeCatalog, RuntimeCatalogError>
{
    serde_json::from_str(RUNTIME_CONTRACTS_JSON).map_err(parse_error)
}

fn parse_error(error: serde_json::Error) -> RuntimeCatalogError {
    RuntimeCatalogError::ParseFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::read_bundled_runtime_contract_catalog;

    #[test]
    fn bundled_runtime_contract_reader_deserializes_contracts() {
        let catalog = read_bundled_runtime_contract_catalog()
            .expect("bundled runtime contracts should deserialize");

        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "runpod-endpoint-comfyui-hidream-o1-dev"),
            "expected bundled ComfyUI endpoint contract"
        );
        assert!(
            catalog
                .contracts
                .iter()
                .any(|contract| contract.id == "provisioner"),
            "expected bundled provisioner contract"
        );
    }
}
