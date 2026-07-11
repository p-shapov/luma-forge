use secrecy::SecretString;

pub struct CreateNetworkVolume {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub size_gb: u64,
}

pub struct StartProvisionerPod {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub required_model_assets: serde_json::Value,
    pub hugging_face_api_key: Option<SecretString>,
}

pub struct CreateTemplate {
    pub workspace_id: String,
    pub image_ref: String,
}

pub struct CreateEndpoint {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub network_volume_id: String,
    pub template_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
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
