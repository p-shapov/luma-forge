#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CatalogRef {
    pub id: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodContractRequirements {
    pub provisioner_contract_ref: CatalogRef,
    pub endpoint_contract_ref: CatalogRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeContractRequirements {
    Runpod(RunpodContractRequirements),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSummary {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub description: String,
    pub required_volume_size_gb: u64,
    pub requires_hugging_face_api_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowDefinition {
    pub summary: WorkflowSummary,
    pub runtime_preset_ref: CatalogRef,
    pub contract_requirements: Vec<RuntimeContractRequirements>,
    pub model_assets: serde_json::Value,
    pub execution_contract: serde_json::Value,
    pub workflow_graph: serde_json::Value,
}

impl WorkflowDefinition {
    pub fn runpod_requirements(
        requirements: &[RuntimeContractRequirements],
    ) -> Option<&RunpodContractRequirements> {
        requirements
            .first()
            .map(|RuntimeContractRequirements::Runpod(value)| value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePreset(pub serde_json::Value);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeContract {
    pub image_ref: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunpodRuntimeDefinition {
    pub runtime_preset: RuntimePreset,
    pub provisioner_contract: RuntimeContract,
    pub endpoint_contract: RuntimeContract,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_runpod_requirements_without_erasing_the_provider_type() {
        let requirements = vec![RuntimeContractRequirements::Runpod(
            RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
            },
        )];

        assert_eq!(
            WorkflowDefinition::runpod_requirements(&requirements),
            Some(&RunpodContractRequirements {
                provisioner_contract_ref: CatalogRef::new("provisioner", "1.0.0"),
                endpoint_contract_ref: CatalogRef::new("endpoint", "1.0.0"),
            })
        );
    }
}
