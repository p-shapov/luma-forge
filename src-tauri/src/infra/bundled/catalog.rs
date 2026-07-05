use std::collections::BTreeMap;

use super::errors::BundledCatalogError;

#[derive(Debug, Clone)]
pub struct BundledCatalog {
    assets: &'static [(&'static str, &'static str)],
}

impl Default for BundledCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl BundledCatalog {
    pub fn new() -> Self {
        Self {
            assets: super::generated::BUNDLED_ASSETS,
        }
    }

    #[cfg(test)]
    pub fn from_assets(assets: &'static [(&'static str, &'static str)]) -> Self {
        Self { assets }
    }

    pub fn assets(&self) -> &'static [(&'static str, &'static str)] {
        self.assets
    }

    pub fn workflow_revision_paths(&self) -> BTreeMap<(String, String), WorkflowRevisionPaths> {
        let mut revisions = BTreeMap::new();
        for (path, _) in self.assets {
            let parts: Vec<&str> = path.split('/').collect();
            if let ["workflows", workflow_id, revision, file] = parts.as_slice() {
                let entry = revisions
                    .entry(((*workflow_id).to_string(), (*revision).to_string()))
                    .or_insert_with(WorkflowRevisionPaths::default);
                entry.set(file, path);
            }
        }
        revisions
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowRevisionPaths {
    pub metadata: Option<String>,
    pub model_assets: Option<String>,
    pub contract_requirements: Option<String>,
    pub execution_contract: Option<String>,
    pub workflow: Option<String>,
}

impl WorkflowRevisionPaths {
    fn set(&mut self, file: &str, path: &str) {
        match file {
            "metadata.json" => self.metadata = Some(path.to_string()),
            "model_assets.json" => self.model_assets = Some(path.to_string()),
            "contract_requirements.json" => self.contract_requirements = Some(path.to_string()),
            "execution_contract.json" => self.execution_contract = Some(path.to_string()),
            "workflow.json" => self.workflow = Some(path.to_string()),
            _ => {}
        }
    }
}

pub fn parse_asset<T: serde::de::DeserializeOwned>(
    path: &str,
    text: &str,
) -> Result<T, BundledCatalogError> {
    serde_json::from_str(text).map_err(|error| BundledCatalogError::CorruptBundledAsset {
        path: path.to_string(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    static FIXTURE_ASSETS: &[(&str, &str)] = &[
        ("workflows/example/1.0.0/metadata.json", "{}"),
        ("workflows/example/1.0.0/model_assets.json", "{}"),
        ("workflows/example/1.0.0/contract_requirements.json", "{}"),
        ("workflows/example/1.0.0/execution_contract.json", "{}"),
        ("workflows/example/1.0.0/workflow.json", "{}"),
        ("runtime_presets/base/1.0.0.json", "{}"),
    ];

    #[test]
    fn workflow_revision_paths_groups_five_workflow_files() {
        let catalog = BundledCatalog::from_assets(FIXTURE_ASSETS);
        let paths = catalog.workflow_revision_paths();
        let revision = paths
            .get(&("example".to_string(), "1.0.0".to_string()))
            .expect("workflow revision should be grouped");

        assert_eq!(
            revision.metadata.as_deref(),
            Some("workflows/example/1.0.0/metadata.json")
        );
        assert_eq!(
            revision.workflow.as_deref(),
            Some("workflows/example/1.0.0/workflow.json")
        );
    }

    #[test]
    fn parse_asset_reports_corrupt_bundled_asset() {
        let error = parse_asset::<serde_json::Value>("fixture.json", "{")
            .expect_err("invalid JSON should fail");

        assert!(matches!(
            error,
            BundledCatalogError::CorruptBundledAsset { .. }
        ));
    }
}
