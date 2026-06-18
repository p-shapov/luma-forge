use crate::domain::runtime_contract::RuntimeCatalog;

use super::RuntimeCatalogError;

pub trait RuntimeCatalogRepository: Send + Sync {
    fn get_runtime_contract_catalog(&self) -> Result<RuntimeCatalog, RuntimeCatalogError>;
}
