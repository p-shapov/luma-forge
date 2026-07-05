use super::super::{catalog::Catalog, models};

#[derive(Debug, Clone)]
pub struct WorkflowRepository {
    catalog: Catalog,
}

impl WorkflowRepository {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Vec<models::WorkflowRevision> {
        self.catalog
            .workflows
            .iter()
            .cloned()
            .map(models::WorkflowRevision::from)
            .collect()
    }

    pub fn find(&self, id: &str, revision: &str) -> Option<models::WorkflowRevision> {
        self.catalog
            .workflows
            .iter()
            .find(|entry| entry.id == id && entry.revision == revision)
            .cloned()
            .map(models::WorkflowRevision::from)
    }
}
