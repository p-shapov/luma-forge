use std::collections::HashMap;

use serde_json::Value;

use super::{parse_document, CatalogEntry, Select};
use crate::infra::bundled::{errors::BundledCatalogError, generated};

#[derive(Debug)]
pub struct Entry {
    pub id: String,
    pub revision: String,
    pub execution_schema: generated::ExecutionSchema,
}

impl Entry {
    pub fn find() -> Select<Self> {
        Select::find()
    }

    pub fn find_by_id(key: (&str, &str)) -> Select<Self> {
        Select::find_by_id(key)
    }
}

impl CatalogEntry for Entry {
    const ENTITY: &'static str = "execution_schema_revision";

    fn from_documents(
        id: String,
        revision: String,
        relative: String,
        mut documents: HashMap<String, Value>,
    ) -> Result<Self, BundledCatalogError> {
        Ok(Self {
            id,
            revision,
            execution_schema: parse_document(&mut documents, &relative, "execution_schema.json")?,
        })
    }
}
