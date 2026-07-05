use super::super::{catalog::Catalog, models};

#[derive(Debug, Clone)]
pub struct RuntimeContractRepository {
    catalog: Catalog,
}

impl RuntimeContractRepository {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Vec<models::RuntimeContractRevision> {
        self.catalog
            .runtime_contracts
            .iter()
            .cloned()
            .map(models::RuntimeContractRevision::from)
            .collect()
    }

    pub fn find(&self, id: &str, revision: &str) -> Option<models::RuntimeContractRevision> {
        self.catalog
            .runtime_contracts
            .iter()
            .find(|entry| entry.id == id && entry.revision == revision)
            .cloned()
            .map(models::RuntimeContractRevision::from)
    }
}
