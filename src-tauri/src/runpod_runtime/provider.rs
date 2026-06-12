use crate::{
    domain::{
        runpod_runtime::{
            RunpodEndpointKeepAliveLimits, RunpodPlacementOptions, RunpodProvisionerStatus,
        },
        workflow_preset::ModelAsset,
    },
    shared::AppFuture,
};

use super::errors::RunpodRuntimeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodNetworkVolumeParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub size_gb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRunpodProvisionerPodParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub network_volume_id: String,
    pub provisioner_image_ref: String,
    pub requires_hugging_face_api_key: bool,
    pub required_model_assets: Vec<ModelAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessTemplateParams {
    pub workspace_id: String,
    pub endpoint_image_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodServerlessEndpointParams {
    pub workspace_id: String,
    pub data_center_id: String,
    pub gpu_type_id: String,
    pub network_volume_id: String,
    pub template_id: String,
    pub keep_alive_limits: Option<RunpodEndpointKeepAliveLimits>,
}

pub trait RunpodRuntimeClient: Send + Sync {
    fn placement_options<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<RunpodPlacementOptions, RunpodRuntimeError>>;

    fn create_network_volume<'a>(
        &'a self,
        params: CreateRunpodNetworkVolumeParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_network_volume<'a>(
        &'a self,
        network_volume_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn start_provisioner_pod<'a>(
        &'a self,
        params: StartRunpodProvisionerPodParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn terminate_provisioner_pod<'a>(
        &'a self,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn get_provisioner_status<'a>(
        &'a self,
        workspace_id: &'a str,
        provisioner_pod_id: &'a str,
    ) -> AppFuture<'a, Result<RunpodProvisionerStatus, RunpodRuntimeError>>;

    fn create_serverless_template<'a>(
        &'a self,
        params: CreateRunpodServerlessTemplateParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn create_serverless_endpoint<'a>(
        &'a self,
        params: CreateRunpodServerlessEndpointParams,
    ) -> AppFuture<'a, Result<String, RunpodRuntimeError>>;

    fn delete_serverless_endpoint<'a>(
        &'a self,
        endpoint_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;

    fn delete_template<'a>(
        &'a self,
        template_id: &'a str,
    ) -> AppFuture<'a, Result<(), RunpodRuntimeError>>;
}
