use crate::application::runtimes::CatalogRef;

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
    pub config: RunpodRuntimeConfig,
    #[diagnostic(show)]
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
