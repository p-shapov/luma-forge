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
