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

use super::contracts::{resolve_contracts, resolve_workflow};
use super::runtime::{
    CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
    CreateRunpodServerlessTemplateParams, RunpodProvisionerStatus, RunpodRuntimeClient,
    StartRunpodProvisionerPodParams,
};
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
    let workflow = resolve_workflow(&workflow_catalog, &workspace.workflow)
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
            RunpodProvisionerStatus::Failed { message } => {
                return Err(super::runtime::map_provider_error(
                    super::errors::RunpodProviderError::ProvisionerWorkerFailed { message },
                ));
            }
            RunpodProvisionerStatus::Pending | RunpodProvisionerStatus::Running => {
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

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::{LifecycleOperationPayload, LifecycleProvisionPayload},
            runpod::RunpodProvisionStep,
            workspace::{WorkspaceRuntime, WorkspaceState},
        },
        provider::runpod::test_support::{
            runpod_client_with_failed_provisioner, runpod_client_with_failure,
            runpod_client_with_state, workspace_with_runpod, RunpodClientFailure,
        },
        shared::ApiError,
        workspace::test_support::runtime_context_for_test,
        workspace::WorkspaceError,
    };

    #[tokio::test]
    async fn provision_creates_all_runpod_resources_and_marks_workspace_ready() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_state();

        let provisioned = super::provision_workspace(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect("provision");

        assert_eq!(provisioned.state, WorkspaceState::Ready);
        let WorkspaceRuntime::Runpod(runtime) = provisioned.runtime;
        assert_eq!(
            runtime.resources.network_volume_id,
            Some("volume".to_string())
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.template_id, Some("template".to_string()));
        assert_eq!(runtime.resources.endpoint_id, Some("endpoint".to_string()));

        let persisted = context
            .find_workspace_for_test("workspace-1")
            .await
            .expect("persisted workspace");
        assert_eq!(persisted.state, WorkspaceState::Ready);
        let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
        assert_eq!(
            runtime.resources.network_volume_id,
            Some("volume".to_string())
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.template_id, Some("template".to_string()));
        assert_eq!(runtime.resources.endpoint_id, Some("endpoint".to_string()));

        let latest = context
            .latest_operation_for_test("workspace-1")
            .await
            .expect("latest operation");
        assert!(matches!(
            latest.payload,
            Some(LifecycleOperationPayload::Provision(
                LifecycleProvisionPayload::Runpod(payload)
            )) if payload.step == Some(RunpodProvisionStep::CreateEndpoint)
        ));
    }

    #[tokio::test]
    async fn provision_stops_at_failed_remote_step() {
        let cases = [
            (
                RunpodClientFailure::CreateNetworkVolume,
                None,
                None,
                None,
                None,
            ),
            (
                RunpodClientFailure::StartProvisionerPod,
                Some("volume"),
                None,
                None,
                None,
            ),
            (
                RunpodClientFailure::GetProvisionerStatus,
                Some("volume"),
                Some("provisioner"),
                None,
                None,
            ),
            (
                RunpodClientFailure::TerminateProvisionerPod,
                Some("volume"),
                Some("provisioner"),
                None,
                None,
            ),
            (
                RunpodClientFailure::CreateServerlessTemplate,
                Some("volume"),
                None,
                None,
                None,
            ),
            (
                RunpodClientFailure::CreateServerlessEndpoint,
                Some("volume"),
                None,
                Some("template"),
                None,
            ),
        ];

        for (
            failure,
            expected_network_volume_id,
            expected_provisioner_pod_id,
            expected_template_id,
            expected_endpoint_id,
        ) in cases
        {
            let workspace_id = format!("workspace-{failure:?}");
            let context = runtime_context_for_test();
            let workspace = workspace_with_runpod(&workspace_id, WorkspaceState::NotProvisioned);
            context.insert_workspace_for_test(workspace.clone()).await;
            let operation = context.create_operation_for_test(&workspace_id).await;
            let runpod_client = runpod_client_with_failure(failure);

            let error = super::provision_workspace(
                context.clone(),
                runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
            .expect_err("provision should fail");

            assert_eq!(
                error,
                WorkspaceError::ProviderApiError(ApiError::RequestFailed {
                    message: failure.message().to_string(),
                })
            );
            let persisted = context
                .find_workspace_for_test(&workspace_id)
                .await
                .expect("persisted workspace");
            assert_eq!(persisted.state, WorkspaceState::NotProvisioned);
            let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
            assert_eq!(
                runtime.resources.network_volume_id,
                expected_network_volume_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.provisioner_pod_id,
                expected_provisioner_pod_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.template_id,
                expected_template_id.map(str::to_string)
            );
            assert_eq!(
                runtime.resources.endpoint_id,
                expected_endpoint_id.map(str::to_string)
            );
        }
    }

    #[tokio::test]
    async fn provision_returns_worker_failure_and_keeps_partial_resources() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_failed_provisioner();

        let error = super::provision_workspace(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect_err("provision should fail");

        assert_eq!(
            error,
            WorkspaceError::ProviderApiError(ApiError::RequestFailed {
                message: "asset_download_failed: download failed".to_string(),
            })
        );
        let persisted = context
            .find_workspace_for_test("workspace-1")
            .await
            .expect("persisted workspace");
        assert_eq!(persisted.state, WorkspaceState::NotProvisioned);
        let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
        assert_eq!(
            runtime.resources.network_volume_id,
            Some("volume".to_string())
        );
        assert_eq!(
            runtime.resources.provisioner_pod_id,
            Some("provisioner".to_string())
        );
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);
    }
}
