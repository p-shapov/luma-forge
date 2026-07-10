pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

use std::collections::HashMap;

use serde_json::Value;

use super::errors::BundledCatalogError;

pub(super) trait CatalogEntry: Sized {
    type Model;

    const CONTRACT: &'static str;

    fn decode(
        id: String,
        revision: String,
        documents: Documents,
    ) -> Result<Self::Model, BundledCatalogError>;
}

pub(super) struct Documents {
    relative: String,
    values: HashMap<String, Value>,
}

impl Documents {
    pub(super) fn new(relative: String, values: HashMap<String, Value>) -> Self {
        Self { relative, values }
    }

    pub(super) fn take<T: serde::de::DeserializeOwned>(
        &mut self,
        name: &str,
    ) -> Result<T, BundledCatalogError> {
        let path = format!("{}/{name}", self.relative);
        let value = self
            .values
            .remove(name)
            .ok_or_else(|| BundledCatalogError::Entry {
                path: path.clone(),
                message: "entry mapping requested an undeclared document".to_string(),
            })?;

        serde_json::from_value(value).map_err(|error| BundledCatalogError::Entry {
            path,
            message: error.to_string(),
        })
    }
}
