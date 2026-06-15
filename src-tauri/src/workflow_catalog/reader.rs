use crate::domain::workflow_preset::WorkflowCatalog;

use super::WorkflowCatalogError;

const WORKFLOW_CATALOG_JSON: &str = include_str!("../../../bundled/workflow-catalog.json");

pub(super) fn read_bundled_workflow_catalog() -> Result<WorkflowCatalog, WorkflowCatalogError> {
    serde_json::from_str(WORKFLOW_CATALOG_JSON).map_err(parse_error)
}

fn parse_error(error: serde_json::Error) -> WorkflowCatalogError {
    WorkflowCatalogError::ParseFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::read_bundled_workflow_catalog;

    #[test]
    fn bundled_workflow_reader_deserializes_workflows() {
        let workflows =
            read_bundled_workflow_catalog().expect("bundled workflows should deserialize");

        assert!(
            workflows
                .workflow_presets
                .iter()
                .any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );

        let revision = workflows
            .workflow_presets
            .iter()
            .find(|workflow| workflow.id == "comfyui-hidream-o1-dev")
            .and_then(|workflow| {
                workflow
                    .revisions
                    .iter()
                    .find(|revision| revision.version == "1.0.0")
            })
            .expect("expected HiDream revision");

        assert_eq!(revision.execution_contract.schema_ref.id, "text-to-image");
        assert_eq!(revision.execution_contract.schema_ref.version, "1.0.0");
        assert_eq!(revision.execution_contract.input_bindings.len(), 3);
    }
}
