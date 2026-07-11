#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProvisionStep {
    CreateNetworkVolume,
    StartProvisionerPod,
    PollProvisioner,
    TerminateProvisionerPod,
    CreateTemplate,
    CreateEndpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodCleanupStep {
    DeleteEndpoint,
    DeleteTemplate,
    TerminateProvisionerPod,
    DeleteNetworkVolume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunpodProgress {
    Provision(RunpodProvisionStep),
    Cleanup(RunpodCleanupStep),
}
