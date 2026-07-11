use crate::application::catalog::{
    CatalogRef, RunpodContractRequirements, RunpodRuntimeDefinition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeCatalogError {
    #[error("runtime catalog is invalid")]
    InvalidCatalog,
    #[error("runtime catalog is unavailable")]
    Unavailable,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeCatalog: Send + Sync {
    async fn resolve(
        &self,
        preset: &CatalogRef,
        requirements: &RunpodContractRequirements,
    ) -> Result<RunpodRuntimeDefinition, RunpodRuntimeCatalogError>;
}
