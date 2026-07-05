use super::super::{catalog::Catalog, models};

#[derive(Debug, Clone)]
pub struct ExecutionSchemaRepository {
    catalog: Catalog,
}

impl ExecutionSchemaRepository {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Vec<models::ExecutionSchemaRevision> {
        self.catalog
            .execution_schemas
            .iter()
            .cloned()
            .map(models::ExecutionSchemaRevision::from)
            .collect()
    }

    pub fn find(&self, id: &str, revision: &str) -> Option<models::ExecutionSchemaRevision> {
        self.catalog
            .execution_schemas
            .iter()
            .find(|entry| entry.id == id && entry.revision == revision)
            .cloned()
            .map(models::ExecutionSchemaRevision::from)
    }
}
