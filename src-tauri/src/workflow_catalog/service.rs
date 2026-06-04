use crate::domain::workflow_preset::WorkflowPreset;

use super::{
    errors::WorkflowCatalogError,
    reader::{
        EndpointContractCatalogReader, ProvisionerContractCatalogReader, WorkflowCatalogReader,
    },
    validation::{validate_runtime_catalog, validate_workflows},
};

#[derive(Debug, Clone)]
pub struct WorkflowCatalogService<W, E, P> {
    workflow_reader: W,
    endpoint_contract_reader: E,
    provisioner_contract_reader: P,
}

impl<W, E, P> WorkflowCatalogService<W, E, P>
where
    W: WorkflowCatalogReader,
    E: EndpointContractCatalogReader,
    P: ProvisionerContractCatalogReader,
{
    pub fn new(
        workflow_reader: W,
        endpoint_contract_reader: E,
        provisioner_contract_reader: P,
    ) -> Self {
        Self {
            workflow_reader,
            endpoint_contract_reader,
            provisioner_contract_reader,
        }
    }

    pub fn get_workflows(&self) -> Result<Vec<WorkflowPreset>, WorkflowCatalogError> {
        let workflows = self.workflow_reader.read_workflows()?;
        let endpoint_contract_catalog = self
            .endpoint_contract_reader
            .read_endpoint_contract_catalog()?;
        let provisioner_contract_catalog = self
            .provisioner_contract_reader
            .read_provisioner_contract_catalog()?;

        validate_runtime_catalog(&endpoint_contract_catalog)?;
        validate_runtime_catalog(&provisioner_contract_catalog)?;
        validate_workflows(
            &workflows,
            &endpoint_contract_catalog,
            &provisioner_contract_catalog,
        )?;

        Ok(workflows)
    }

    pub fn get_workflow_by_id(
        &self,
        workflow_id: &str,
    ) -> Result<Option<WorkflowPreset>, WorkflowCatalogError> {
        Ok(self
            .get_workflows()?
            .into_iter()
            .find(|workflow| workflow.id == workflow_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow_catalog::bundled_reader::{
        BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
        BundledWorkflowCatalogReader,
    };

    fn bundled_service() -> WorkflowCatalogService<
        BundledWorkflowCatalogReader,
        BundledEndpointContractCatalogReader,
        BundledProvisionerContractCatalogReader,
    > {
        WorkflowCatalogService::new(
            BundledWorkflowCatalogReader,
            BundledEndpointContractCatalogReader,
            BundledProvisionerContractCatalogReader,
        )
    }

    #[test]
    fn get_workflows_returns_bundled_workflows() {
        let workflows = bundled_service()
            .get_workflows()
            .expect("bundled workflows should be valid");

        assert!(
            workflows
                .iter()
                .any(|workflow| workflow.id == "comfyui-hidream-o1-dev"),
            "expected bundled HiDream workflow"
        );
    }

    #[test]
    fn get_workflow_by_id_returns_matching_workflow() {
        let workflow = bundled_service()
            .get_workflow_by_id("comfyui-hidream-o1-dev")
            .expect("bundled workflows should be valid")
            .expect("known workflow should be present");

        assert_eq!(workflow.id, "comfyui-hidream-o1-dev");
    }

    #[test]
    fn get_workflow_by_id_returns_none_for_unknown_workflow() {
        let workflow = bundled_service()
            .get_workflow_by_id("unknown-workflow")
            .expect("bundled workflows should be valid");

        assert_eq!(workflow, None);
    }
}
