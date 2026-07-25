use serde::{Deserialize, Serialize};

use crate::application::runtimes::{Runtime, RuntimeKind, RuntimeProvider, RuntimeState};

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDto {
    pub state: RuntimeStateDto,
    pub provider: RuntimeProviderDto,
}

impl From<Runtime> for RuntimeDto {
    fn from(value: Runtime) -> Self {
        Self {
            state: value.state.into(),
            provider: value.provider.into(),
        }
    }
}

#[derive(
    luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type,
)]
#[serde(
    tag = "runtimeKind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RuntimeProviderDto {
    Runpod {
        datacenter_id: String,
        gpu_id: String,
        volume_size_gb: u64,
    },
}

impl From<RuntimeProvider> for RuntimeProviderDto {
    fn from(value: RuntimeProvider) -> Self {
        match value {
            RuntimeProvider::Runpod(runtime) => Self::Runpod {
                datacenter_id: runtime.config.datacenter_id,
                gpu_id: runtime.config.gpu_id,
                volume_size_gb: runtime.config.volume_size_gb,
            },
        }
    }
}

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKindDto {
    Runpod,
}

impl From<RuntimeKind> for RuntimeKindDto {
    fn from(value: RuntimeKind) -> Self {
        match value {
            RuntimeKind::Runpod => Self::Runpod,
        }
    }
}

#[derive(
    luma_diagnostics::DiagnosticDebug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    specta::Type,
)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStateDto {
    Provisioning,
    Ready,
    CleaningUp,
    Failed,
}

impl From<RuntimeState> for RuntimeStateDto {
    fn from(value: RuntimeState) -> Self {
        match value {
            RuntimeState::Provisioning => Self::Provisioning,
            RuntimeState::Ready => Self::Ready,
            RuntimeState::CleaningUp => Self::CleaningUp,
            RuntimeState::Failed => Self::Failed,
        }
    }
}
