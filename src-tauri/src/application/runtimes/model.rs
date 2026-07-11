use super::runpod::{RunpodProgress, RunpodRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Runpod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    Runpod(RunpodRuntime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProgress {
    Runpod(RunpodProgress),
}

pub trait RuntimeModel: Clone + Send + Sync + 'static {
    fn workspace_id(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn into_runtime(self) -> Runtime;
}

#[cfg(test)]
pub(crate) fn progress_fixture() -> RuntimeProgress {
    RuntimeProgress::Runpod(RunpodProgress::Provision(
        super::runpod::RunpodProvisionStep::CreateNetworkVolume,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::runpod::{
        RunpodProgress, RunpodProvisionStep, RunpodRuntime, RunpodRuntimeConfig,
        RunpodRuntimeResources, RunpodRuntimeState,
    };

    #[test]
    fn runpod_model_converts_without_erasing_its_provider_type() {
        let runtime = RunpodRuntime {
            workspace_id: "workspace-1".into(),
            state: RunpodRuntimeState::Ready,
            config: RunpodRuntimeConfig {
                datacenter_id: "dc-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 19,
            },
            resources: RunpodRuntimeResources::default(),
        };

        assert_eq!(runtime.workspace_id(), "workspace-1");
        assert_eq!(runtime.kind(), RuntimeKind::Runpod);
        assert_eq!(runtime.clone().into_runtime(), Runtime::Runpod(runtime));
    }

    #[test]
    fn runtime_dispatch_owns_provider_progress() {
        assert_eq!(
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
        );
    }
}
