use secrecy::SecretString;

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct IdentityRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct IdentityResponse {
    #[diagnostic(show)]
    pub user_id: Option<String>,
    #[diagnostic(show)]
    pub email: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct PlacementRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct PlacementResponse {
    #[diagnostic(show)]
    pub gpu_types: Option<Vec<Option<PlacementGpuType>>>,
    #[diagnostic(show)]
    pub datacenters: Option<Vec<Option<PlacementDatacenter>>>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct PlacementGpuType {
    #[diagnostic(show)]
    pub id: Option<String>,
    #[diagnostic(show)]
    pub display_name: Option<String>,
    #[diagnostic(show)]
    pub memory_gb: Option<i64>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct PlacementDatacenter {
    #[diagnostic(show)]
    pub id: Option<String>,
    #[diagnostic(show)]
    pub name: Option<String>,
    #[diagnostic(show)]
    pub gpu_availability: Option<Vec<Option<PlacementGpuAvailability>>>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct PlacementGpuAvailability {
    #[diagnostic(show)]
    pub gpu_type_id: Option<String>,
    #[diagnostic(show)]
    pub available: Option<bool>,
    #[diagnostic(show)]
    pub stock_status: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateNetworkVolumeRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub size_gb: i64,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateNetworkVolumeResponse {
    #[diagnostic(show)]
    pub id: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct DeleteNetworkVolumeRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(show)]
    pub id: String,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreatePodRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(redact)]
    pub hugging_face_credential: Option<SecretString>,
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub provisioner_image_ref: String,
    #[diagnostic(show)]
    pub network_volume_id: String,
    pub required_model_assets: serde_json::Value,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreatePodResponse {
    #[diagnostic(show)]
    pub id: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct ProvisionerStatusRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub pod_id: String,
}

#[derive(crate::diagnostics::DiagnosticDebug, serde::Deserialize)]
pub struct ProvisionerStatusResponse {
    #[diagnostic(show)]
    pub status: String,
    #[diagnostic(show)]
    pub error: Option<ProvisionerFailure>,
}

#[derive(crate::diagnostics::DiagnosticDebug, serde::Deserialize)]
pub struct ProvisionerFailure {
    #[diagnostic(show)]
    pub code: String,
    #[diagnostic(show)]
    pub message: String,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateEndpointRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub gpu_id: String,
    #[diagnostic(show)]
    pub network_volume_id: String,
    #[diagnostic(show)]
    pub template_id: String,
    #[diagnostic(show)]
    pub workers_min: i64,
    #[diagnostic(show)]
    pub workers_max: i64,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateEndpointResponse {
    #[diagnostic(show)]
    pub id: Option<String>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateTemplateRequest {
    #[diagnostic(redact)]
    pub credential: SecretString,
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub image_ref: String,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateTemplateResponse {
    #[diagnostic(show)]
    pub id: Option<String>,
}

macro_rules! delete_request {
    ($name:ident) => {
        #[derive(crate::diagnostics::DiagnosticDebug)]
        pub struct $name {
            #[diagnostic(redact)]
            pub credential: SecretString,
            #[diagnostic(show)]
            pub id: String,
        }
    };
}

delete_request!(DeletePodRequest);
delete_request!(DeleteTemplateRequest);
delete_request!(DeleteEndpointRequest);
