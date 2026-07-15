use serde::{Deserialize, Serialize};

use crate::application::runtimes::RuntimeOperationKind;

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum RunpodProvisionStep {
    #[serde(rename = "create_network_volume")]
    CreateNetworkVolume,
    #[serde(rename = "start_provisioner_pod")]
    StartProvisionerPod,
    #[serde(rename = "poll_provisioner")]
    PollProvisioner,
    #[serde(rename = "terminate_provisioner_pod")]
    TerminateProvisionerPod,
    #[serde(rename = "create_template")]
    CreateTemplate,
    #[serde(rename = "create_endpoint")]
    CreateEndpoint,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum RunpodCleanupStep {
    #[serde(rename = "delete_endpoint")]
    DeleteEndpoint,
    #[serde(rename = "delete_template")]
    DeleteTemplate,
    #[serde(rename = "terminate_provisioner_pod")]
    TerminateProvisionerPod,
    #[serde(rename = "delete_network_volume")]
    DeleteNetworkVolume,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(tag = "operation", content = "step", deny_unknown_fields)]
pub enum RunpodProgress {
    #[serde(rename = "provision")]
    Provision(#[diagnostic(show)] RunpodProvisionStep),
    #[serde(rename = "cleanup")]
    Cleanup(#[diagnostic(show)] RunpodCleanupStep),
}

impl RunpodProgress {
    pub fn operation_kind(self) -> RuntimeOperationKind {
        match self {
            Self::Provision(_) => RuntimeOperationKind::Provision,
            Self::Cleanup(_) => RuntimeOperationKind::Cleanup,
        }
    }

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
