use crate::application::runtimes::CatalogRef;

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodContractRequirements {
    #[diagnostic(show)]
    pub provisioner_contract_ref: CatalogRef,
    #[diagnostic(show)]
    pub endpoint_contract_ref: CatalogRef,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeDefinition {
    #[diagnostic(show)]
    pub provisioner_image_ref: String,
    #[diagnostic(show)]
    pub endpoint_image_ref: String,
}
