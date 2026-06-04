use std::{future::Future, pin::Pin};

use crate::domain::{
    provider::GpuCloudProviderId,
    workspace::{
        RemoteEndpointSnapshot, RemoteProvisionerSnapshot, RemoteProvisionerStatus,
        RemoteVolumeSnapshot,
    },
};

use super::errors::{
    CreateEndpointError, CreateVolumeError, DeleteEndpointError, DeleteVolumeError,
    GetProvisionerStatusError, ObserveEndpointError, ObserveProvisionerError, ObserveVolumeError,
    StartProvisionerError, TerminateProvisionerError,
};

pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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
    ) -> ProviderFuture<'a, Result<RemoteVolumeSnapshot, CreateVolumeError>>;

    fn delete_volume<'a>(
        &'a self,
        params: DeleteVolumeParams,
    ) -> ProviderFuture<'a, Result<(), DeleteVolumeError>>;

    fn observe_volume<'a>(
        &'a self,
        params: ObserveVolumeParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteVolumeSnapshot>, ObserveVolumeError>>;
}

pub trait RemoteProvisionerProvider {
    fn start_provisioner<'a>(
        &'a self,
        params: StartProvisionerParams,
    ) -> ProviderFuture<'a, Result<RemoteProvisionerSnapshot, StartProvisionerError>>;

    fn terminate_provisioner<'a>(
        &'a self,
        params: TerminateProvisionerParams,
    ) -> ProviderFuture<'a, Result<(), TerminateProvisionerError>>;

    fn observe_provisioner<'a>(
        &'a self,
        params: ObserveProvisionerParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteProvisionerSnapshot>, ObserveProvisionerError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        params: GetProvisionerStatusParams,
    ) -> ProviderFuture<'a, Result<RemoteProvisionerStatus, GetProvisionerStatusError>>;
}

pub trait RemoteEndpointProvider {
    fn create_endpoint<'a>(
        &'a self,
        params: CreateEndpointParams,
    ) -> ProviderFuture<'a, Result<RemoteEndpointSnapshot, CreateEndpointError>>;

    fn delete_endpoint<'a>(
        &'a self,
        params: DeleteEndpointParams,
    ) -> ProviderFuture<'a, Result<(), DeleteEndpointError>>;

    fn observe_endpoint<'a>(
        &'a self,
        params: ObserveEndpointParams,
    ) -> ProviderFuture<'a, Result<Option<RemoteEndpointSnapshot>, ObserveEndpointError>>;
}

pub trait RemoteWorkspaceProvider:
    RemoteVolumeProvider + RemoteProvisionerProvider + RemoteEndpointProvider + Send + Sync
{
    fn provider_id(&self) -> GpuCloudProviderId;
}
