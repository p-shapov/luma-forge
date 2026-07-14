use serde::{Deserialize, Serialize};

use crate::application::runtimes::{CatalogRef, RuntimeOperationKind};

pub const RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB: u64 = 4_000;

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodPlacement {
    #[diagnostic(show)]
    pub max_volume_size_gb: u64,
    #[diagnostic(show)]
    pub datacenters: Vec<RunpodPlacementDatacenter>,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodPlacementDatacenter {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub gpus: Vec<RunpodPlacementGpu>,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct RunpodPlacementGpu {
    #[diagnostic(show)]
    pub id: String,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub vram_gb: u64,
}

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

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntimeConfig {
    #[diagnostic(show)]
    #[serde(rename = "datacenter_id")]
    pub datacenter_id: String,
    #[diagnostic(show)]
    #[serde(rename = "gpu_id")]
    pub gpu_id: String,
    #[diagnostic(show)]
    #[serde(rename = "volume_size_gb")]
    pub volume_size_gb: u64,
}

#[derive(
    crate::diagnostics::DiagnosticDebug, Clone, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntimeResources {
    #[diagnostic(show)]
    #[serde(rename = "network_volume_id")]
    pub network_volume_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "provisioner_pod_id")]
    pub provisioner_pod_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "template_id")]
    pub template_id: Option<String>,
    #[diagnostic(show)]
    #[serde(rename = "endpoint_id")]
    pub endpoint_id: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunpodRuntime {
    #[diagnostic(show)]
    #[serde(rename = "config")]
    pub config: RunpodRuntimeConfig,
    #[diagnostic(show)]
    #[serde(rename = "resources")]
    pub resources: RunpodRuntimeResources,
}

impl RunpodRuntime {
    pub fn new_provisioning(config: RunpodRuntimeConfig) -> Self {
        Self {
            config,
            resources: RunpodRuntimeResources::default(),
        }
    }
}
