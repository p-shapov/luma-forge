use std::{sync::Arc, time::Duration};

use crate::{
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationId, LifecycleOperationState},
        runpod::{RunpodPlacementPlan, RunpodProvisionStep, RunpodRuntime},
        workspace::{Workspace, WorkspaceId, WorkspaceRuntime, WorkspaceState},
    },
    lifecycle_journal::LifecycleJournalRepository,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::super::provider::RunpodProvisionerStatus;
use super::{
    super::{
        catalogs::RunpodRuntimeCatalogServices,
        contracts::{
            RunpodContractResolver, RunpodRuntimeContracts, RunpodWorkflowResolved,
            RunpodWorkflowResolver,
        },
        errors::{invalid_runtime_state_message, RunpodRuntimeError},
        events::RunpodRuntimeEventSink,
        provider::{
            CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
            CreateRunpodServerlessTemplateParams, RunpodRuntimeClient,
            StartRunpodProvisionerPodParams,
        },
    },
    helpers::{
        load_running_operation, mark_operation_failed, mark_operation_state, mark_running_step,
        mark_workspace_failed, persist_workspace, RunpodWorkspaceFailure,
    },
};

// Keep polling until the worker container is reachable; 12 * 5s = 60s warm-up window.
const MAX_PROVISIONER_STARTUP_PROBE_ATTEMPTS: u32 = 12;

struct ProvisioningInputs {
    placement: RunpodPlacementPlan,
    workflow: RunpodWorkflowResolved,
    contracts: RunpodRuntimeContracts,
}

struct ProvisioningStepContext<'a, W, L> {
    workspace_repository: &'a W,
    lifecycle_journal: &'a L,
    operation: &'a LifecycleOperation,
    runpod_client: &'a dyn RunpodRuntimeClient,
    event_sink: &'a Arc<dyn RunpodRuntimeEventSink>,
}

#[tracing::instrument(
    name = "runpod_lifecycle",
    skip_all,
    fields(
        operation_kind = "provision",
        operation_id = %operation_id,
        workspace_id = tracing::field::Empty
    )
)]
pub(crate) async fn run_once<W, L>(
    operation_id: &LifecycleOperationId,
    workspace_repository: &W,
    lifecycle_journal: &L,
    catalogs: &RunpodRuntimeCatalogServices,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    provisioner_poll_interval: Duration,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    let operation = load_running_operation(lifecycle_journal, operation_id).await?;
    tracing::Span::current().record(
        "workspace_id",
        tracing::field::display(&operation.workspace_id),
    );
    let mut workspace = match workspace_repository
        .find_workspace_by_id(&operation.workspace_id)
        .await
    {
        Ok(Some(workspace)) => workspace,
        Ok(None) => {
            let error = invalid_runtime_state_message("workspace was not found");
            mark_operation_failed(
                lifecycle_journal,
                event_sink,
                &operation,
                RunpodProvisionStep::CreateNetworkVolume,
                &error,
            )
            .await?;
            return Ok(());
        }
        Err(error) => {
            let error = RunpodRuntimeError::from(error);
            mark_operation_failed(
                lifecycle_journal,
                event_sink,
                &operation,
                RunpodProvisionStep::CreateNetworkVolume,
                &error,
            )
            .await?;
            return Ok(());
        }
    };

    let mut failed_step = RunpodProvisionStep::CreateNetworkVolume;
    let result = async {
        let inputs = resolve_provisioning_inputs(&workspace, catalogs)?;
        let step_context = ProvisioningStepContext {
            workspace_repository,
            lifecycle_journal,
            operation: &operation,
            runpod_client,
            event_sink,
        };

        failed_step = RunpodProvisionStep::CreateNetworkVolume;
        let volume_id =
            create_network_volume(&mut workspace, &step_context, &inputs.placement).await?;

        failed_step = RunpodProvisionStep::StartProvisionerPod;
        let provisioner_id =
            start_provisioner_pod(&mut workspace, &step_context, &inputs, &volume_id).await?;

        failed_step = RunpodProvisionStep::PollProvisioner;
        let provisioner_failed = wait_for_provisioner(
            lifecycle_journal,
            event_sink,
            &operation,
            step_context.runpod_client,
            &workspace.id,
            &provisioner_id,
            provisioner_poll_interval,
        )
        .await?;

        failed_step = RunpodProvisionStep::TerminateProvisionerPod;
        terminate_provisioner_pod(&mut workspace, &step_context, &provisioner_id).await?;

        if provisioner_failed {
            return Err(RunpodRuntimeError::ProvisionerWorkerFailed {
                message: "provisioner worker failed".to_string(),
            });
        }

        failed_step = RunpodProvisionStep::CreateTemplate;
        let template_id =
            create_serverless_template(&mut workspace, &step_context, &inputs.contracts).await?;

        failed_step = RunpodProvisionStep::CreateEndpoint;
        create_serverless_endpoint(
            &mut workspace,
            &step_context,
            &inputs.placement,
            &volume_id,
            &template_id,
        )
        .await?;

        workspace.state = WorkspaceState::Ready;
        persist_workspace(workspace_repository, event_sink, &workspace).await?;

        Ok::<(), RunpodRuntimeError>(())
    }
    .await;

    match result {
        Ok(_) => {
            mark_operation_state(
                lifecycle_journal,
                event_sink,
                &operation,
                LifecycleOperationState::Completed,
                RunpodProvisionStep::CreateEndpoint,
            )
            .await?;
        }
        Err(error) => {
            mark_workspace_failed(
                &mut workspace,
                workspace_repository,
                event_sink,
                RunpodWorkspaceFailure::Provision,
            )
            .await?;
            mark_operation_failed(
                lifecycle_journal,
                event_sink,
                &operation,
                failed_step,
                &error,
            )
            .await?;
        }
    }

    Ok(())
}

fn resolve_provisioning_inputs(
    workspace: &Workspace,
    catalogs: &RunpodRuntimeCatalogServices,
) -> Result<ProvisioningInputs, RunpodRuntimeError> {
    let workflows = catalogs
        .workflow_catalog
        .get_workflow_catalog()
        .map_err(RunpodRuntimeError::from)?;
    let workflow = RunpodWorkflowResolver::resolve(&workflows, &workspace.workflow)
        .ok_or_else(|| invalid_runtime_state_message("workflow reference was not found"))?;
    let contracts = RunpodContractResolver::resolve(&workflow, &catalogs.runtime_catalog)?;

    Ok(ProvisioningInputs {
        placement: runpod_runtime(workspace).placement.clone(),
        workflow,
        contracts,
    })
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "create_network_volume",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id,
        datacenter_id = %placement.data_center_id,
        volume_size_gb = placement.volume_size_gb
    )
)]
async fn create_network_volume<W, L>(
    workspace: &mut Workspace,
    context: &ProvisioningStepContext<'_, W, L>,
    placement: &RunpodPlacementPlan,
) -> Result<String, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        RunpodProvisionStep::CreateNetworkVolume,
    )
    .await?;

    let volume_id = context
        .runpod_client
        .create_network_volume(CreateRunpodNetworkVolumeParams {
            workspace_id: workspace.id.clone(),
            data_center_id: placement.data_center_id.clone(),
            size_gb: placement.volume_size_gb,
        })
        .await?;
    persist_runpod_runtime_update(
        workspace,
        context.workspace_repository,
        context.event_sink,
        |runtime| {
            runtime.resources.network_volume_id = Some(volume_id.clone());
        },
    )
    .await?;

    Ok(volume_id)
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "start_provisioner_pod",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id,
        datacenter_id = %inputs.placement.data_center_id,
        requires_hugging_face_api_key = inputs.workflow.requires_hugging_face_api_key
    )
)]
async fn start_provisioner_pod<W, L>(
    workspace: &mut Workspace,
    context: &ProvisioningStepContext<'_, W, L>,
    inputs: &ProvisioningInputs,
    volume_id: &str,
) -> Result<String, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        RunpodProvisionStep::StartProvisionerPod,
    )
    .await?;

    let provisioner_id = context
        .runpod_client
        .start_provisioner_pod(StartRunpodProvisionerPodParams {
            workspace_id: workspace.id.clone(),
            data_center_id: inputs.placement.data_center_id.clone(),
            network_volume_id: volume_id.to_string(),
            provisioner_image_ref: inputs.contracts.provisioner_contract.image_ref.clone(),
            requires_hugging_face_api_key: inputs.workflow.requires_hugging_face_api_key,
            required_model_assets: inputs.workflow.required_model_assets.clone(),
        })
        .await?;
    persist_runpod_runtime_update(
        workspace,
        context.workspace_repository,
        context.event_sink,
        |runtime| {
            runtime.resources.provisioner_pod_id = Some(provisioner_id.clone());
        },
    )
    .await?;

    Ok(provisioner_id)
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "terminate_provisioner_pod",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id,
        provisioner_pod_id = %provisioner_id
    )
)]
async fn terminate_provisioner_pod<W, L>(
    workspace: &mut Workspace,
    context: &ProvisioningStepContext<'_, W, L>,
    provisioner_id: &str,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        RunpodProvisionStep::TerminateProvisionerPod,
    )
    .await?;

    context
        .runpod_client
        .terminate_provisioner_pod(provisioner_id)
        .await?;
    persist_runpod_runtime_update(
        workspace,
        context.workspace_repository,
        context.event_sink,
        |runtime| {
            runtime.resources.provisioner_pod_id = None;
        },
    )
    .await
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "create_template",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id
    )
)]
async fn create_serverless_template<W, L>(
    workspace: &mut Workspace,
    context: &ProvisioningStepContext<'_, W, L>,
    contracts: &RunpodRuntimeContracts,
) -> Result<String, RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        RunpodProvisionStep::CreateTemplate,
    )
    .await?;

    let template_id = context
        .runpod_client
        .create_serverless_template(CreateRunpodServerlessTemplateParams {
            workspace_id: workspace.id.clone(),
            endpoint_image_ref: contracts.endpoint_contract.image_ref.clone(),
        })
        .await?;
    persist_runpod_runtime_update(
        workspace,
        context.workspace_repository,
        context.event_sink,
        |runtime| {
            runtime.resources.template_id = Some(template_id.clone());
        },
    )
    .await?;

    Ok(template_id)
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "create_endpoint",
        operation_id = %context.operation.operation_id,
        workspace_id = %context.operation.workspace_id,
        datacenter_id = %placement.data_center_id,
        gpu_type_id = %placement.gpu_type_id,
        network_volume_id = %volume_id,
        template_id = %template_id
    )
)]
async fn create_serverless_endpoint<W, L>(
    workspace: &mut Workspace,
    context: &ProvisioningStepContext<'_, W, L>,
    placement: &RunpodPlacementPlan,
    volume_id: &str,
    template_id: &str,
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
    L: LifecycleJournalRepository,
{
    mark_running_step(
        context.lifecycle_journal,
        context.event_sink,
        context.operation,
        RunpodProvisionStep::CreateEndpoint,
    )
    .await?;

    let endpoint_id = match context
        .runpod_client
        .create_serverless_endpoint(CreateRunpodServerlessEndpointParams {
            workspace_id: workspace.id.clone(),
            data_center_id: placement.data_center_id.clone(),
            gpu_type_id: placement.gpu_type_id.clone(),
            network_volume_id: volume_id.to_string(),
            template_id: template_id.to_string(),
            keep_alive_limits: None,
        })
        .await
    {
        Ok(endpoint_id) => endpoint_id,
        Err(error) => {
            discard_template_after_endpoint_failure(
                workspace,
                context.workspace_repository,
                context.runpod_client,
                context.event_sink,
                template_id,
            )
            .await;
            return Err(error);
        }
    };

    persist_runpod_runtime_update(
        workspace,
        context.workspace_repository,
        context.event_sink,
        |runtime| {
            runtime.resources.endpoint_id = Some(endpoint_id);
        },
    )
    .await
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "discard_template_after_endpoint_failure",
        workspace_id = %workspace.id,
        template_id = %template_id
    )
)]
async fn discard_template_after_endpoint_failure<W>(
    workspace: &mut Workspace,
    workspace_repository: &W,
    runpod_client: &dyn RunpodRuntimeClient,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    template_id: &str,
) where
    W: WorkspaceCatalogRepository,
{
    if runpod_client.delete_template(template_id).await.is_err() {
        return;
    }

    let _ = persist_runpod_runtime_update(workspace, workspace_repository, event_sink, |runtime| {
        runtime.resources.template_id = None;
    })
    .await;
}

async fn persist_runpod_runtime_update<W>(
    workspace: &mut Workspace,
    workspace_repository: &W,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    update: impl FnOnce(&mut RunpodRuntime),
) -> Result<(), RunpodRuntimeError>
where
    W: WorkspaceCatalogRepository,
{
    update(runpod_runtime_mut(workspace));
    *workspace = persist_workspace(workspace_repository, event_sink, workspace).await?;
    Ok(())
}

fn runpod_runtime(workspace: &Workspace) -> &RunpodRuntime {
    match &workspace.runtime {
        WorkspaceRuntime::Runpod(runtime) => runtime,
    }
}

fn runpod_runtime_mut(workspace: &mut Workspace) -> &mut RunpodRuntime {
    match &mut workspace.runtime {
        WorkspaceRuntime::Runpod(runtime) => runtime,
    }
}

#[tracing::instrument(
    name = "runpod_lifecycle_step",
    skip_all,
    fields(
        operation_kind = "provision",
        step = "poll_provisioner",
        operation_id = %operation.operation_id,
        workspace_id = %workspace_id,
        provisioner_pod_id = %provisioner_id
    )
)]
async fn wait_for_provisioner<L>(
    lifecycle_journal: &L,
    event_sink: &Arc<dyn RunpodRuntimeEventSink>,
    operation: &LifecycleOperation,
    provider: &dyn RunpodRuntimeClient,
    workspace_id: &WorkspaceId,
    provisioner_id: &str,
    provisioner_poll_interval: Duration,
) -> Result<bool, RunpodRuntimeError>
where
    L: LifecycleJournalRepository,
{
    let mut has_seen_initial_status = false;
    let mut startup_probe_attempts = 0u32;

    loop {
        mark_running_step(
            lifecycle_journal,
            event_sink,
            operation,
            RunpodProvisionStep::PollProvisioner,
        )
        .await?;

        match provider
            .get_provisioner_status(workspace_id, provisioner_id)
            .await
        {
            Ok(RunpodProvisionerStatus::Succeeded) => return Ok(false),
            Ok(RunpodProvisionerStatus::Failed) => return Ok(true),
            Ok(
                RunpodProvisionerStatus::Pending
                | RunpodProvisionerStatus::Starting
                | RunpodProvisionerStatus::Running,
            ) => {
                has_seen_initial_status = true;
                startup_probe_attempts = 0;
                sleep_between_provisioner_polls(provisioner_poll_interval).await;
            }
            Err(RunpodRuntimeError::ProvisionerWorkerUnavailable { .. })
                if should_retry_initial_provisioner_probe(
                    has_seen_initial_status,
                    startup_probe_attempts,
                    provisioner_poll_interval,
                ) =>
            {
                startup_probe_attempts += 1;
                sleep_between_provisioner_polls(provisioner_poll_interval).await;
            }
            Err(error) => return Err(error),
        }
    }
}

fn should_retry_initial_provisioner_probe(
    has_seen_initial_status: bool,
    startup_probe_attempts: u32,
    provisioner_poll_interval: Duration,
) -> bool {
    !has_seen_initial_status
        && !provisioner_poll_interval.is_zero()
        && startup_probe_attempts < MAX_PROVISIONER_STARTUP_PROBE_ATTEMPTS
}

async fn sleep_between_provisioner_polls(provisioner_poll_interval: Duration) {
    if !provisioner_poll_interval.is_zero() {
        tokio::time::sleep(provisioner_poll_interval).await;
    }
}
