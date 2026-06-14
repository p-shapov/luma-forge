use crate::{runtime_catalog::RuntimeCatalogService, workflow_catalog::WorkflowCatalogService};

#[derive(Debug, Clone)]
pub(crate) struct RunpodRuntimeCatalogServices {
    pub(crate) workflow_catalog: WorkflowCatalogService,
    pub(crate) runtime_catalog: RuntimeCatalogService,
}

impl RunpodRuntimeCatalogServices {
    pub(crate) fn new(
        workflow_catalog: WorkflowCatalogService,
        runtime_catalog: RuntimeCatalogService,
    ) -> Self {
        Self {
            workflow_catalog,
            runtime_catalog,
        }
    }
}
