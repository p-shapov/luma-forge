use crate::application::workspace::RuntimeKind;

use super::runpod::RunpodRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Runtime {
    Runpod(RunpodRuntime),
}

pub trait RuntimeModel: Clone + Send + Sync + 'static {
    fn workspace_id(&self) -> &str;
    fn kind(&self) -> RuntimeKind;
    fn into_runtime(self) -> Runtime;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::runtimes::runpod::{
        RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeResources, RunpodRuntimeState,
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
}
