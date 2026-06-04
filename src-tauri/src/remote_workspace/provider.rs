use crate::domain::{
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
pub struct ObserveVolumeParams {
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartProvisionerParams {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_id: String,
    pub provisioner_image_ref: String,
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminateProvisionerParams {
    pub workspace_id: String,
    pub provisioner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveProvisionerParams {
    pub workspace_id: String,
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
    pub mount_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserveEndpointParams {
    pub workspace_id: String,
    pub endpoint_id: Option<String>,
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

    fn observe_volume<'a>(
        &'a self,
        params: ObserveVolumeParams,
    ) -> AppFuture<'a, Result<Option<RemoteVolumeSnapshot>, RemoteWorkspaceError>>;
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

    fn observe_provisioner<'a>(
        &'a self,
        params: ObserveProvisionerParams,
    ) -> AppFuture<'a, Result<Option<RemoteProvisionerSnapshot>, RemoteWorkspaceError>>;

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

    fn observe_endpoint<'a>(
        &'a self,
        params: ObserveEndpointParams,
    ) -> AppFuture<'a, Result<Option<RemoteEndpointSnapshot>, RemoteWorkspaceError>>;
}

pub trait RemoteWorkspaceProvider:
    RemoteVolumeProvider + RemoteProvisionerProvider + RemoteEndpointProvider + Send + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
