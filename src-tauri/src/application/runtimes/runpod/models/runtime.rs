use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    #[serde(rename = "provision_operation_id")]
    pub provision_operation_id: Uuid,
    #[diagnostic(show)]
    #[serde(rename = "config")]
    pub config: RunpodRuntimeConfig,
    #[diagnostic(show)]
    #[serde(rename = "resources")]
    pub resources: RunpodRuntimeResources,
}

impl RunpodRuntime {
    pub fn new_provisioning(provision_operation_id: Uuid, config: RunpodRuntimeConfig) -> Self {
        Self {
            provision_operation_id,
            config,
            resources: RunpodRuntimeResources::default(),
        }
    }
}
