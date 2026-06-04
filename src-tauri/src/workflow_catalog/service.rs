#[derive(Debug, Clone)]
pub struct WorkflowCatalogService<W, E, P> {
    workflow_reader: W,
    endpoint_contract_reader: E,
    provisioner_contract_reader: P,
}

impl<W, E, P> WorkflowCatalogService<W, E, P> {
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
}
