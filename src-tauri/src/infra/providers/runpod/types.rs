#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodIdentity {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodPlacementOptions {
    pub gpu_types: Vec<RunpodGpuType>,
    pub datacenters: Vec<RunpodDatacenter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuType {
    pub id: String,
    pub display_name: String,
    pub memory_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodDatacenter {
    pub id: String,
    pub name: String,
    pub gpu_availability: Vec<RunpodGpuAvailability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunpodGpuAvailability {
    pub gpu_type_id: String,
    pub available: Option<bool>,
    pub stock_status: Option<String>,
}
