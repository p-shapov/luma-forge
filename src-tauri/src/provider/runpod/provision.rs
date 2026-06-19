use std::time::Duration;

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationPayload, LifecycleProvisionPayload,
        },
        runpod::{RunpodLifecycleProvisionPayload, RunpodProvisionStep, RunpodRuntime},
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    runtime_catalog::BundledRuntimeCatalogRepository,
    workflow_catalog::{BundledWorkflowCatalogRepository, WorkflowCatalogRepository},
    workspace::{errors::invalid_state, WorkspaceError, WorkspaceRuntimeContext},
};

use super::client::{
    CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
    CreateRunpodServerlessTemplateParams, RunpodProvisionerStatus, RunpodRuntimeClient,
    StartRunpodProvisionerPodParams,
};
use super::contracts::{resolve_contracts, RunpodWorkflowResolver};
const PROVISIONER_POLL_INTERVAL: Duration = Duration::from_secs(5);

fn provision_payload(step: RunpodProvisionStep) -> LifecycleOperationPayload {
    LifecycleOperationPayload::Provision(LifecycleProvisionPayload::Runpod(
        RunpodLifecycleProvisionPayload { step: Some(step) },
    ))
}

async fn mark_step(
    context: &WorkspaceRuntimeContext<'_>,
    operation: &mut LifecycleOperation,
    step: RunpodProvisionStep,
) -> Result<(), WorkspaceError> {
    operation.payload = Some(provision_payload(step));
    *operation = context.persist_operation(operation.clone()).await?;
    Ok(())
}

pub async fn provision_workspace(
    context: WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    mut operation: LifecycleOperation,
    mut workspace: Workspace,
) -> Result<Workspace, WorkspaceError> {
    let workflow_catalog = BundledWorkflowCatalogRepository::new().get_workflow_catalog()?;
    let workflow = RunpodWorkflowResolver::resolve(&workflow_catalog, &workspace.workflow)
        .ok_or_else(|| invalid_state("workflow reference was not found"))?;
    let runtime_catalog = BundledRuntimeCatalogRepository::new();
    let contracts = resolve_contracts(&workflow, &runtime_catalog)?;
    let placement = runpod_workspace(&workspace).placement.clone();

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::CreateNetworkVolume,
    )
    .await?;
    let network_volume_id = runpod_client
        .create_network_volume(CreateRunpodNetworkVolumeParams {
            workspace_id: workspace.id.clone(),
            data_center_id: placement.data_center_id.clone(),
            size_gb: placement.volume_size_gb,
        })
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(&mut workspace)
        .resources
        .network_volume_id = Some(network_volume_id.clone());
    workspace = context.persist_workspace(workspace).await?;

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::StartProvisionerPod,
    )
    .await?;
    let provisioner_pod_id = runpod_client
        .start_provisioner_pod(StartRunpodProvisionerPodParams {
            workspace_id: workspace.id.clone(),
            data_center_id: placement.data_center_id.clone(),
            network_volume_id: network_volume_id.clone(),
            provisioner_image_ref: contracts.provisioner_contract.image_ref,
            requires_hugging_face_api_key: workflow.requires_hugging_face_api_key,
            required_model_assets: workflow.required_model_assets,
        })
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(&mut workspace)
        .resources
        .provisioner_pod_id = Some(provisioner_pod_id.clone());
    workspace = context.persist_workspace(workspace).await?;

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::PollProvisioner,
    )
    .await?;
    loop {
        match runpod_client
            .get_provisioner_status(&workspace.id, &provisioner_pod_id)
            .await
            .map_err(super::runtime::map_provider_error)?
        {
            RunpodProvisionerStatus::Succeeded => break,
            RunpodProvisionerStatus::Failed => {
                return Err(super::runtime::map_provider_error(
                    super::errors::RunpodProviderError::ProvisionerWorkerFailed {
                        message: "provisioner worker failed".to_string(),
                    },
                ));
            }
            RunpodProvisionerStatus::Pending
            | RunpodProvisionerStatus::Starting
            | RunpodProvisionerStatus::Running => {
                tokio::time::sleep(PROVISIONER_POLL_INTERVAL).await;
            }
        }
    }

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::TerminateProvisionerPod,
    )
    .await?;
    runpod_client
        .terminate_provisioner_pod(&provisioner_pod_id)
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(&mut workspace)
        .resources
        .provisioner_pod_id = None;
    workspace = context.persist_workspace(workspace).await?;

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::CreateTemplate,
    )
    .await?;
    let template_id = runpod_client
        .create_serverless_template(CreateRunpodServerlessTemplateParams {
            workspace_id: workspace.id.clone(),
            endpoint_image_ref: contracts.endpoint_contract.image_ref,
        })
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(&mut workspace).resources.template_id = Some(template_id.clone());
    workspace = context.persist_workspace(workspace).await?;

    mark_step(
        &context,
        &mut operation,
        RunpodProvisionStep::CreateEndpoint,
    )
    .await?;
    let endpoint_id = runpod_client
        .create_serverless_endpoint(CreateRunpodServerlessEndpointParams {
            workspace_id: workspace.id.clone(),
            data_center_id: placement.data_center_id,
            gpu_type_id: placement.gpu_type_id,
            network_volume_id,
            template_id,
        })
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(&mut workspace).resources.endpoint_id = Some(endpoint_id);
    workspace.state = WorkspaceState::Ready;
    context.persist_workspace(workspace).await
}

fn runpod_workspace(workspace: &Workspace) -> &RunpodRuntime {
    let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
    runtime
}

fn runpod_workspace_mut(workspace: &mut Workspace) -> &mut RunpodRuntime {
    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
    runtime
}
