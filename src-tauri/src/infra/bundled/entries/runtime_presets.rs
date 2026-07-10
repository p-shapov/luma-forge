use super::{CatalogEntry, Documents};
use crate::infra::bundled::{errors::BundledCatalogError, generated, Catalog};

pub struct Entry;

#[derive(Debug)]
pub struct Model {
    pub id: String,
    pub revision: String,
    pub runtime_preset: generated::RuntimePreset,
}

impl Entry {
    pub async fn all(catalog: &Catalog) -> Result<Vec<Model>, BundledCatalogError> {
        catalog.all::<Self>().await
    }

    pub async fn get(
        catalog: &Catalog,
        key: (&str, &str),
    ) -> Result<Option<Model>, BundledCatalogError> {
        catalog.get::<Self>(key).await
    }
}

impl CatalogEntry for Entry {
    type Model = Model;

    const CONTRACT: &'static str = "catalog/contracts/runtime_preset_revision";

    fn decode(
        id: String,
        revision: String,
        mut documents: Documents,
    ) -> Result<Model, BundledCatalogError> {
        Ok(Model {
            id,
            revision,
            runtime_preset: documents.take("runtime_preset")?,
        })
    }
}
