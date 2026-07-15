use crate::application::runtimes::runpod::RunpodContractRequirements;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRef {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub revision: String,
}

impl CatalogRef {
    pub fn new(id: impl Into<String>, revision: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            revision: revision.into(),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub enum RuntimeContractRequirements {
    Runpod(#[diagnostic(show)] RunpodContractRequirements),
}

impl RuntimeContractRequirements {
    pub fn as_runpod(&self) -> Option<&RunpodContractRequirements> {
        match self {
            Self::Runpod(value) => Some(value),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub revision: String,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub description: String,
    #[diagnostic(show)]
    pub required_volume_size_gb: u64,
    #[diagnostic(show)]
    pub requires_hugging_face_api_key: bool,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    #[diagnostic(show)]
    pub summary: WorkflowSummary,
    #[diagnostic(show)]
    pub runtime_preset_ref: CatalogRef,
    #[diagnostic(show)]
    pub contract_requirements: Vec<RuntimeContractRequirements>,
    pub model_assets: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub workflow_graph: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::runpod::RunpodContractRequirements;

    use super::*;

    #[test]
    fn runtime_requirements_expose_their_runpod_value() {
        let expected = RunpodContractRequirements {
            provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
            endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
        };
        let requirements = RuntimeContractRequirements::Runpod(expected.clone());

        assert_eq!(requirements.as_runpod(), Some(&expected));
    }
}
