use crate::application::runtimes::{CatalogRef, Runtime, RuntimeKind, RuntimeModel};

use super::RunpodRuntimeError;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodContractRequirements {
    #[diagnostic(show)]
    pub provisioner_contract_ref: CatalogRef,
    #[diagnostic(show)]
    pub endpoint_contract_ref: CatalogRef,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeDefinition {
    #[diagnostic(show)]
    pub provisioner_image_ref: String,
    #[diagnostic(show)]
    pub endpoint_image_ref: String,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProvisionStep {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProgress {
    Provision(#[diagnostic(show)] RunpodProvisionStep),
    Cleanup(#[diagnostic(show)] RunpodCleanupStep),
}

impl RunpodProgress {
    pub fn provision_step(self) -> Option<RunpodProvisionStep> {
        match self {
            Self::Provision(step) => Some(step),
            Self::Cleanup(_) => None,
        }
    }

    pub fn cleanup_step(self) -> Option<RunpodCleanupStep> {
        match self {
            Self::Provision(_) => None,
            Self::Cleanup(step) => Some(step),
        }
    }
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodRuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeConfig {
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub gpu_id: String,
    #[diagnostic(show)]
    pub volume_size_gb: u64,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Default, PartialEq, Eq)]
pub struct RunpodRuntimeResources {
    #[diagnostic(show)]
    pub network_volume_id: Option<String>,
    #[diagnostic(show)]
    pub provisioner_pod_id: Option<String>,
    #[diagnostic(show)]
    pub template_id: Option<String>,
    #[diagnostic(show)]
    pub endpoint_id: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodRuntime {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub state: RunpodRuntimeState,
    #[diagnostic(show)]
    pub config: RunpodRuntimeConfig,
    #[diagnostic(show)]
    pub resources: RunpodRuntimeResources,
}

impl RuntimeModel for RunpodRuntime {
    fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Runpod
    }

    fn into_runtime(self) -> Runtime {
        Runtime::Runpod(self)
    }
}

impl RunpodRuntime {
    pub fn new_provisioning(workspace_id: impl Into<String>, config: RunpodRuntimeConfig) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            state: RunpodRuntimeState::Provisioning,
            config,
            resources: RunpodRuntimeResources::default(),
        }
    }

    pub fn begin_provision(&mut self) -> Result<(), RunpodRuntimeError> {
        Err(match self.state {
            RunpodRuntimeState::Ready => RunpodRuntimeError::AlreadyProvisioned,
            RunpodRuntimeState::Failed => RunpodRuntimeError::RuntimeFailed,
            RunpodRuntimeState::Provisioning | RunpodRuntimeState::CleaningUp => {
                RunpodRuntimeError::OperationInProgress
            }
        })
    }

    pub fn begin_cleanup(&mut self) -> Result<(), RunpodRuntimeError> {
        match self.state {
            RunpodRuntimeState::Ready | RunpodRuntimeState::Failed => {
                self.state = RunpodRuntimeState::CleaningUp;
                Ok(())
            }
            RunpodRuntimeState::Provisioning | RunpodRuntimeState::CleaningUp => {
                Err(RunpodRuntimeError::OperationInProgress)
            }
        }
    }

    pub fn mark_ready(&mut self) -> Result<(), RunpodRuntimeError> {
        if self.state != RunpodRuntimeState::Provisioning {
            return Err(RunpodRuntimeError::InvalidTransition);
        }
        self.state = RunpodRuntimeState::Ready;
        Ok(())
    }

    pub fn mark_failed(&mut self) -> Result<(), RunpodRuntimeError> {
        match self.state {
            RunpodRuntimeState::Provisioning | RunpodRuntimeState::CleaningUp => {
                self.state = RunpodRuntimeState::Failed;
                Ok(())
            }
            RunpodRuntimeState::Ready | RunpodRuntimeState::Failed => {
                Err(RunpodRuntimeError::InvalidTransition)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_in(state: RunpodRuntimeState) -> RunpodRuntime {
        RunpodRuntime {
            workspace_id: "workspace-1".to_owned(),
            state,
            config: RunpodRuntimeConfig {
                datacenter_id: "datacenter-1".to_owned(),
                gpu_id: "gpu-1".to_owned(),
                volume_size_gb: 100,
            },
            resources: RunpodRuntimeResources::default(),
        }
    }

    #[test]
    fn ready_runtime_can_start_cleanup_but_cannot_provision_again() {
        let mut runtime = runtime_in(RunpodRuntimeState::Ready);

        assert_eq!(
            runtime.begin_provision(),
            Err(RunpodRuntimeError::AlreadyProvisioned)
        );
        runtime.begin_cleanup().unwrap();
        assert_eq!(runtime.state, RunpodRuntimeState::CleaningUp);
    }

    #[test]
    fn failed_runtime_requires_cleanup() {
        let mut runtime = runtime_in(RunpodRuntimeState::Failed);

        assert_eq!(
            runtime.begin_provision(),
            Err(RunpodRuntimeError::RuntimeFailed)
        );
        assert_eq!(runtime.begin_cleanup(), Ok(()));
    }

    #[test]
    fn active_transition_rejects_another_operation() {
        for state in [
            RunpodRuntimeState::Provisioning,
            RunpodRuntimeState::CleaningUp,
        ] {
            let mut runtime = runtime_in(state);
            assert_eq!(
                runtime.begin_provision(),
                Err(RunpodRuntimeError::OperationInProgress)
            );
        }
    }
}
