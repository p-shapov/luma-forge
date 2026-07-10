pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

use std::{collections::HashMap, marker::PhantomData};

use serde_json::Value;

use super::{catalog::Catalog, errors::BundledCatalogError};

pub trait CatalogEntry: Sized {
    const ENTITY: &'static str;

    fn from_documents(
        id: String,
        revision: String,
        relative: String,
        documents: HashMap<String, Value>,
    ) -> Result<Self, BundledCatalogError>;
}

pub struct Select<E> {
    key: Option<(String, String)>,
    entity: PhantomData<E>,
}

impl<E: CatalogEntry> Select<E> {
    pub(crate) fn find() -> Self {
        Self {
            key: None,
            entity: PhantomData,
        }
    }

    pub(crate) fn find_by_id((id, revision): (&str, &str)) -> Self {
        Self {
            key: Some((id.to_string(), revision.to_string())),
            entity: PhantomData,
        }
    }

    pub async fn all(self, catalog: &Catalog) -> Result<Vec<E>, BundledCatalogError> {
        catalog
            .query()
            .await?
            .load::<E>(self.key.as_ref(), false)
            .await
    }

    pub async fn one(self, catalog: &Catalog) -> Result<Option<E>, BundledCatalogError> {
        Ok(catalog
            .query()
            .await?
            .load::<E>(self.key.as_ref(), true)
            .await?
            .into_iter()
            .next())
    }
}

fn parse_document<T: serde::de::DeserializeOwned>(
    documents: &mut HashMap<String, Value>,
    relative: &str,
    name: &str,
) -> Result<T, BundledCatalogError> {
    let path = format!("catalog/entries/{relative}/{name}");
    let value = documents
        .remove(name)
        .ok_or_else(|| BundledCatalogError::Contract {
            path: path.clone(),
            message: "missing required file".to_string(),
        })?;

    serde_json::from_value(value).map_err(|error| BundledCatalogError::Contract {
        path,
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{execution_schemas, runtime_contracts, runtime_presets, workflows};
    use crate::infra::bundled::Catalog;

    #[tokio::test]
    async fn entries_list_and_find_catalog_records() {
        let catalog = Catalog::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("../new_bundled"));

        assert_eq!(
            workflows::Entry::find().all(&catalog).await.unwrap().len(),
            1
        );
        assert_eq!(
            runtime_contracts::Entry::find()
                .all(&catalog)
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            runtime_presets::Entry::find()
                .all(&catalog)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            execution_schemas::Entry::find()
                .all(&catalog)
                .await
                .unwrap()
                .len(),
            1
        );

        let workflow = workflows::Entry::find_by_id(("comfyui-hidream-o1-dev", "1.0.0"))
            .one(&catalog)
            .await
            .unwrap()
            .expect("workflow should exist");
        assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
        assert_eq!(workflow.revision, "1.0.0");

        assert!(workflows::Entry::find_by_id(("missing", "1.0.0"))
            .one(&catalog)
            .await
            .unwrap()
            .is_none());
    }
}
