use std::sync::Arc;

use super::{
    ports::{RuntimeOperationRepository, RuntimeOperationRepositoryError},
    RuntimeOperation,
};

#[derive(Clone)]
pub struct RuntimeOperationQueryService {
    operations: Arc<dyn RuntimeOperationRepository>,
}

impl RuntimeOperationQueryService {
    pub fn new(operations: Arc<dyn RuntimeOperationRepository>) -> Self {
        Self { operations }
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn page(
        &self,
        #[diagnostic(show)] workspace_id: Option<&str>,
        #[diagnostic(show)] offset: u64,
        #[diagnostic(show)] limit: u64,
    ) -> Result<(Vec<RuntimeOperation>, u64), RuntimeOperationRepositoryError> {
        self.operations.page(workspace_id, offset, limit).await
    }

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn running(&self) -> Result<Vec<RuntimeOperation>, RuntimeOperationRepositoryError> {
        self.operations.running().await
    }
}
