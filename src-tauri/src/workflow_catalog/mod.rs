pub mod reader;
pub mod service;

mod validation;

pub use reader::{
    BundledEndpointContractCatalogReader, BundledProvisionerContractCatalogReader,
    BundledWorkflowCatalogReader, EndpointContractCatalogReader, ProvisionerContractCatalogReader,
    WorkflowCatalogReader,
};
pub use service::WorkflowCatalogService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowCatalogError {
    ParseFailed,
    ValidationFailed,
}

pub type WorkflowCatalogResult<T> = Result<T, WorkflowCatalogError>;
