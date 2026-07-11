use super::super::CatalogRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodContractRequirements {
    pub provisioner_contract_ref: CatalogRef,
    pub endpoint_contract_ref: CatalogRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeDefinition {
    pub provisioner_image_ref: String,
    pub endpoint_image_ref: String,
}
