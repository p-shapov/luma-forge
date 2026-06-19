use std::sync::Arc;

use crate::domain::workspace::WorkspaceRuntime as WorkspaceRuntimeDomain;

use super::{errors::WorkspaceError, runtime::WorkspaceRuntime};

#[derive(Clone)]
pub struct WorkspaceRuntimeRegistry {
    runpod: Arc<dyn WorkspaceRuntime>,
}

impl WorkspaceRuntimeRegistry {
    pub fn new(runpod: Arc<dyn WorkspaceRuntime>) -> Self {
        Self { runpod }
    }

    pub fn runtime_for(
        &self,
        runtime: &WorkspaceRuntimeDomain,
    ) -> Result<Arc<dyn WorkspaceRuntime>, WorkspaceError> {
        match runtime {
            WorkspaceRuntimeDomain::Runpod(_) => Ok(self.runpod.clone()),
        }
    }
}
