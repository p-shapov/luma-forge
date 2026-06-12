use crate::{
    domain::{
        provisioned_remote::GpuCloudProviderId,
        provisioned_remote::ProvisionedRemoteProvisionerStatus,
        provisioned_remote::{RemoteEndpointKeepAliveLimits, RemotePlacementOptions},
        workflow_preset::ModelAsset,
    },
    shared::AppFuture,
};

use super::errors::ProvisionedRemoteError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateVolumeParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteVolumeParams {
    pub workspace_id: String,
    pub volume_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProvisionerParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub provisioner_image_ref: String,
    pub requires_hugging_face_api_key: bool,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminateProvisionerParams {
    pub workspace_id: String,
    pub provisioner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetProvisionerStatusParams {
    pub workspace_id: String,
    pub provisioner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEndpointParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub endpoint_image_ref: String,
    pub keep_alive_limits: Option<RemoteEndpointKeepAliveLimits>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: String,
}

pub trait ProvisionedRemotePlacementOptionsProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, ProvisionedRemoteError>>;
}

pub trait ProvisionedRemoteVolumeProvider {
    fn create_volume<'a>(
        &'a self,
        params: CreateVolumeParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>>;

    fn delete_volume<'a>(
        &'a self,
        params: DeleteVolumeParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
}

pub trait ProvisionedRemoteProvisionerProvider {
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>>;

    fn terminate_provisioner<'a>(
        &'a self,
        params: TerminateProvisionerParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        params: GetProvisionerStatusParams,
    ) -> AppFuture<'a, Result<ProvisionedRemoteProvisionerStatus, ProvisionedRemoteError>>;
}

pub trait ProvisionedRemoteEndpointProvider {
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> AppFuture<'a, Result<String, ProvisionedRemoteError>>;

    fn delete_endpoint<'a>(
        &'a self,
        params: DeleteEndpointParams,
    ) -> AppFuture<'a, Result<(), ProvisionedRemoteError>>;
}

pub trait ProvisionedRemoteProvider:
    ProvisionedRemotePlacementOptionsProvider
    + ProvisionedRemoteVolumeProvider
    + ProvisionedRemoteProvisionerProvider
    + ProvisionedRemoteEndpointProvider
    + Send
    + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
