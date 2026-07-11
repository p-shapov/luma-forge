use crate::application::{
    runtimes::{Runtime, RuntimeModel},
    workspace::{RuntimeKind, WorkspaceStatus},
};

use super::RunpodRuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodRuntimeState {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntimeConfig {
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunpodRuntimeResources {
    pub network_volume_id: Option<String>,
    pub provisioner_pod_id: Option<String>,
    pub template_id: Option<String>,
    pub endpoint_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodRuntime {
    pub workspace_id: String,
    pub state: RunpodRuntimeState,
    pub config: RunpodRuntimeConfig,
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

impl From<RunpodRuntimeState> for WorkspaceStatus {
    fn from(state: RunpodRuntimeState) -> Self {
        match state {
            RunpodRuntimeState::Provisioning => Self::Provisioning,
            RunpodRuntimeState::Ready => Self::Ready,
            RunpodRuntimeState::CleaningUp => Self::CleaningUp,
            RunpodRuntimeState::Failed => Self::Failed,
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
