use secrecy::SecretString;
use uuid::Uuid;

use super::super::RunpodPlacement;

#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunpodResourceKind {
    NetworkVolume,
    ProvisionerPod,
    Template,
    Endpoint,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct ObserveNetworkVolume {
    pub name: String,
    pub datacenter_id: String,
    pub size_gb: u64,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct ObserveProvisionerPod {
    pub name: String,
    pub network_volume_id: String,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct ObserveTemplate {
    pub name: String,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub struct ObserveEndpoint {
    pub name: String,
    pub gpu_id: String,
    pub network_volume_id: String,
    pub template_id: String,
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, PartialEq, Eq)]
pub enum RunpodResourceObservation {
    Absent,
    Found(String),
    Ambiguous(Vec<String>),
}

impl RunpodResourceKind {
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::NetworkVolume => "volume",
            Self::ProvisionerPod => "provisioner",
            Self::Template => "template",
            Self::Endpoint => "endpoint",
        }
    }
}

pub(crate) fn resource_name(
    workspace_id: &str,
    provision_operation_id: Uuid,
    kind: RunpodResourceKind,
) -> String {
    format!(
        "luma-forge-{workspace_id}-{provision_operation_id}-{}",
        kind.suffix()
    )
}

#[derive(luma_diagnostics::DiagnosticDebug)]
pub struct CreateNetworkVolume {
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub size_gb: u64,
}

#[derive(luma_diagnostics::DiagnosticDebug)]
pub struct StartProvisionerPod {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub name: String,
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

#[derive(luma_diagnostics::DiagnosticDebug)]
pub struct CreateTemplate {
    #[diagnostic(show)]
    pub name: String,
    #[diagnostic(show)]
    pub image_ref: String,
}

#[derive(luma_diagnostics::DiagnosticDebug)]
pub struct CreateEndpoint {
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
}

#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunpodRuntimeProviderError {
    #[error("runtime provider rejected the credential")]
    Unauthorized,
    #[error("runtime provider is unavailable")]
    Unavailable,
    #[error("runtime provider create outcome is unknown")]
    CreateOutcomeUnknown,
    #[error("runtime provider resource observation is unavailable")]
    ObserveUnavailable,
    #[error("runtime provisioner failed")]
    ProvisionerFailed,
}

#[async_trait::async_trait]
pub trait RunpodRuntimeProvider: Send + Sync {
    async fn placement(
        &self,
        api_key: &SecretString,
    ) -> Result<RunpodPlacement, RunpodRuntimeProviderError>;
    async fn observe_network_volume(
        &self,
        api_key: &SecretString,
        command: ObserveNetworkVolume,
    ) -> Result<RunpodResourceObservation, RunpodRuntimeProviderError>;
    async fn observe_provisioner_pod(
        &self,
        api_key: &SecretString,
        command: ObserveProvisionerPod,
    ) -> Result<RunpodResourceObservation, RunpodRuntimeProviderError>;
    async fn observe_template(
        &self,
        api_key: &SecretString,
        command: ObserveTemplate,
    ) -> Result<RunpodResourceObservation, RunpodRuntimeProviderError>;
    async fn observe_endpoint(
        &self,
        api_key: &SecretString,
        command: ObserveEndpoint,
    ) -> Result<RunpodResourceObservation, RunpodRuntimeProviderError>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_names_include_provision_operation_id() {
        let provision_operation_id = Uuid::from_u128(1);

        for (kind, expected) in [
            (
                RunpodResourceKind::NetworkVolume,
                "luma-forge-workspace-1-00000000-0000-0000-0000-000000000001-volume",
            ),
            (
                RunpodResourceKind::ProvisionerPod,
                "luma-forge-workspace-1-00000000-0000-0000-0000-000000000001-provisioner",
            ),
            (
                RunpodResourceKind::Template,
                "luma-forge-workspace-1-00000000-0000-0000-0000-000000000001-template",
            ),
            (
                RunpodResourceKind::Endpoint,
                "luma-forge-workspace-1-00000000-0000-0000-0000-000000000001-endpoint",
            ),
        ] {
            assert_eq!(
                resource_name("workspace-1", provision_operation_id, kind),
                expected
            );
        }

        assert_ne!(
            resource_name(
                "workspace-1",
                provision_operation_id,
                RunpodResourceKind::NetworkVolume,
            ),
            resource_name(
                "workspace-1",
                Uuid::from_u128(2),
                RunpodResourceKind::NetworkVolume,
            )
        );
    }
}
