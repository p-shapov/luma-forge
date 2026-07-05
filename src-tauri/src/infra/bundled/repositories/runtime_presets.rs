use super::super::{catalog::Catalog, models};

#[derive(Debug, Clone)]
pub struct RuntimePresetRepository {
    catalog: Catalog,
}

impl RuntimePresetRepository {
    pub fn new(catalog: Catalog) -> Self {
        Self { catalog }
    }

    pub fn list(&self) -> Vec<models::RuntimePresetRevision> {
        self.catalog
            .runtime_presets
            .iter()
            .cloned()
            .map(models::RuntimePresetRevision::from)
            .collect()
    }

    pub fn find(&self, id: &str, revision: &str) -> Option<models::RuntimePresetRevision> {
        self.catalog
            .runtime_presets
            .iter()
            .find(|entry| entry.id == id && entry.revision == revision)
            .cloned()
            .map(models::RuntimePresetRevision::from)
    }
}
