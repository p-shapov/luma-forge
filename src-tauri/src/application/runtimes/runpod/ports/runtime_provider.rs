use secrecy::SecretString;

use super::super::RunpodPlacement;

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateNetworkVolume {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub size_gb: u64,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct StartProvisionerPod {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub network_volume_id: String,
    #[diagnostic(show)]
    pub provisioner_image_ref: String,
    pub required_model_assets: serde_json::Value,
    #[diagnostic(redact)]
    pub hugging_face_api_key: Option<SecretString>,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateTemplate {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub image_ref: String,
}

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct CreateEndpoint {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub gpu_id: String,
    #[diagnostic(show)]
    pub network_volume_id: String,
    #[diagnostic(show)]
    pub template_id: String,
}

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeProviderError {
    #[error("runtime provider rejected the credential")]
    Unauthorized,
    #[error("runtime provider is unavailable")]
    Unavailable,
    #[error("runtime provisioner failed")]
    ProvisionerFailed,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeProvider: Send + Sync {
    async fn placement(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacement, RunpodRuntimeProviderError>;
    async fn create_network_volume(
        &self,
        api_key: &SecretString,
        command: CreateNetworkVolume,
    ) -> Result<String, RunpodRuntimeProviderError>;
    async fn start_provisioner_pod(
        &self,
        api_key: &SecretString,
        command: StartProvisionerPod,
    ) -> Result<String, RunpodRuntimeProviderError>;
    async fn wait_for_provisioner(
        &self,
        api_key: &SecretString,
        workspace_id: &str,
        pod_id: &str,
    ) -> Result<(), RunpodRuntimeProviderError>;
    async fn terminate_provisioner_pod(
        &self,
        api_key: &SecretString,
        pod_id: &str,
    ) -> Result<(), RunpodRuntimeProviderError>;
    async fn create_template(
        &self,
        api_key: &SecretString,
        command: CreateTemplate,
    ) -> Result<String, RunpodRuntimeProviderError>;
    async fn create_endpoint(
        &self,
        api_key: &SecretString,
        command: CreateEndpoint,
    ) -> Result<String, RunpodRuntimeProviderError>;
    async fn delete_endpoint(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodRuntimeProviderError>;
    async fn delete_template(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodRuntimeProviderError>;
    async fn delete_network_volume(
        &self,
        api_key: &SecretString,
        id: &str,
    ) -> Result<(), RunpodRuntimeProviderError>;
}
