use serde::de::DeserializeOwned;

use super::{errors::BundledCatalogError, generated};

pub mod execution_schemas;
pub mod runtime_contracts;
pub mod runtime_presets;
pub mod workflows;

fn parse_asset<T: DeserializeOwned>(path: &str, text: &str) -> Result<T, BundledCatalogError> {
    serde_json::from_str(text).map_err(|error| corrupt(path, error.to_string()))
}

fn corrupt(path: &str, message: impl Into<String>) -> BundledCatalogError {
    BundledCatalogError::CorruptBundledAsset {
        path: path.to_string(),
        message: message.into(),
    }
}
