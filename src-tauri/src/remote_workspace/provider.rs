use crate::domain::{
    placement::{RemoteEndpointKeepAliveLimits, RemotePlacementOptions},
    provider::GpuCloudProviderId,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
        RemoteVolumeSnapshot,
    },
};
use crate::shared::AppFuture;

use super::errors::RemoteWorkspaceError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateVolumeParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub size_bytes: u64,
    pub mount_path: String,
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
    pub mount_path: String,
    pub requires_hugging_face_api_key: bool,
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
    pub status_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEndpointParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub endpoint_image_ref: String,
    pub mount_path: String,
    pub keep_alive_limits: Option<RemoteEndpointKeepAliveLimits>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: String,
}

pub trait RemotePlacementOptionsProvider {
    fn get_provider_placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RemotePlacementOptions, RemoteWorkspaceError>>;
}

pub trait RemoteVolumeProvider {
    fn create_volume<'a>(
        &'a self,
        params: CreateVolumeParams,
    ) -> AppFuture<'a, Result<RemoteVolumeSnapshot, RemoteWorkspaceError>>;

    fn delete_volume<'a>(
        &'a self,
        params: DeleteVolumeParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;
}

pub trait RemoteProvisionerProvider {
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerSnapshot, RemoteWorkspaceError>>;

    fn terminate_provisioner<'a>(
        &'a self,
        params: TerminateProvisionerParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        params: GetProvisionerStatusParams,
    ) -> AppFuture<'a, Result<RemoteProvisionerStatus, RemoteWorkspaceError>>;
}

pub trait RemoteEndpointProvider {
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> AppFuture<'a, Result<RemoteEndpointSnapshot, RemoteWorkspaceError>>;

    fn delete_endpoint<'a>(
        &'a self,
        params: DeleteEndpointParams,
    ) -> AppFuture<'a, Result<(), RemoteWorkspaceError>>;
}

pub trait RemoteWorkspaceProvider:
    RemotePlacementOptionsProvider
    + RemoteVolumeProvider
    + RemoteProvisionerProvider
    + RemoteEndpointProvider
    + Send
    + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
