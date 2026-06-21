use std::time::{Duration, Instant};

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationPayload, LifecycleProvisionPayload,
        },
        runpod::{RunpodLifecycleProvisionPayload, RunpodProvisionStep, RunpodRuntime},
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    runtime_catalog::RuntimeCatalogRepository,
    workflow_catalog::WorkflowCatalogRepository,
    workspace::{errors::invalid_state, WorkspaceError, WorkspaceRuntimeContext},
};

use super::contracts::{resolve_contracts, resolve_workflow};
use super::runtime::{
    CreateRunpodNetworkVolumeParams, CreateRunpodServerlessEndpointParams,
    CreateRunpodServerlessTemplateParams, RunpodProvisionerStatus, RunpodRuntimeClient,
    StartRunpodProvisionerPodParams,
};
const PROVISIONER_POLL_INTERVAL: Duration = Duration::from_secs(5);
const PROVISIONER_STARTUP_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const PROVISIONER_UNAVAILABLE_RETRY_LIMIT_AFTER_STARTUP: usize = 3;

fn provision_payload(step: RunpodProvisionStep) -> LifecycleOperationPayload {
    LifecycleOperationPayload::Provision(LifecycleProvisionPayload::Runpod(
        RunpodLifecycleProvisionPayload { step: Some(step) },
    ))
}

async fn mark_step(
    context: &WorkspaceRuntimeContext<'_>,
    operation: &mut LifecycleOperation,
    workspace_id: &str,
    step: RunpodProvisionStep,
) -> Result<(), WorkspaceError> {
    log::info!(
        workspace_id = workspace_id,
        operation_id = operation.operation_id.as_str(),
        step:? = step;
        "runpod provision step started"
    );
    operation.payload = Some(provision_payload(step.clone()));
    *operation = context.persist_operation(operation.clone()).await?;
    log::info!(
        workspace_id = workspace_id,
        operation_id = operation.operation_id.as_str(),
        step:? = step;
        "runpod provision step persisted"
    );
    Ok(())
}

pub async fn provision_workspace(
    context: WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    workflow_catalog: &dyn WorkflowCatalogRepository,
    runtime_catalog: &dyn RuntimeCatalogRepository,
    mut operation: LifecycleOperation,
    mut workspace: Workspace,
) -> Result<Workspace, WorkspaceError> {
    let workflow_catalog = workflow_catalog.get_workflow_catalog()?;
    let workflow = resolve_workflow(&workflow_catalog, &workspace.workflow)
        .ok_or_else(|| invalid_state("workflow reference was not found"))?;
    let contracts = resolve_contracts(&workflow, runtime_catalog)?;
    let placement = runpod_workspace(&workspace).placement.clone();

    mark_step(
        &context,
        &mut operation,
        workspace.id.as_str(),
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
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::CreateNetworkVolume,
        network_volume_id = network_volume_id.as_str();
        "runpod provision step completed"
    );

    mark_step(
        &context,
        &mut operation,
        workspace.id.as_str(),
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
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::StartProvisionerPod,
        provisioner_pod_id = provisioner_pod_id.as_str(),
        network_volume_id = network_volume_id.as_str();
        "runpod provision step completed"
    );

    mark_step(
        &context,
        &mut operation,
        workspace.id.as_str(),
        RunpodProvisionStep::PollProvisioner,
    )
    .await?;
    let provisioner_startup_started_at = Instant::now();
    let mut provisioner_status_responded = false;
    let mut consecutive_unavailable_status_polls = 0;
    loop {
        let status = match runpod_client
            .get_provisioner_status(&workspace.id, &provisioner_pod_id)
            .await
        {
            Ok(status) => status,
            Err(super::errors::RunpodProviderError::ProvisionerWorkerUnavailable { .. }) => {
                if provisioner_status_responded {
                    consecutive_unavailable_status_polls += 1;
                }
                if provisioner_worker_unavailable_timed_out(
                    provisioner_status_responded,
                    consecutive_unavailable_status_polls,
                    provisioner_startup_started_at,
                    Instant::now(),
                ) {
                    let message = if provisioner_status_responded {
                        "provisioner worker unavailable after startup retry limit"
                    } else {
                        "provisioner worker startup timed out"
                    };
                    terminate_provisioner_pod(
                        &context,
                        runpod_client,
                        &mut operation,
                        &mut workspace,
                        &provisioner_pod_id,
                    )
                    .await?;
                    return Err(super::runtime::map_provider_error(
                        super::errors::RunpodProviderError::ProvisionerWorkerUnavailable {
                            message: message.to_string(),
                        },
                    ));
                }
                tokio::time::sleep(PROVISIONER_POLL_INTERVAL).await;
                continue;
            }
            Err(error) => {
                terminate_provisioner_pod(
                    &context,
                    runpod_client,
                    &mut operation,
                    &mut workspace,
                    &provisioner_pod_id,
                )
                .await?;
                return Err(super::runtime::map_provider_error(error));
            }
        };
        provisioner_status_responded = true;
        consecutive_unavailable_status_polls = 0;

        match status {
            RunpodProvisionerStatus::Succeeded => break,
            RunpodProvisionerStatus::Failed { message } => {
                terminate_provisioner_pod(
                    &context,
                    runpod_client,
                    &mut operation,
                    &mut workspace,
                    &provisioner_pod_id,
                )
                .await?;
                return Err(super::runtime::map_provider_error(
                    super::errors::RunpodProviderError::ProvisionerWorkerFailed { message },
                ));
            }
            RunpodProvisionerStatus::Pending | RunpodProvisionerStatus::Running => {
                tokio::time::sleep(PROVISIONER_POLL_INTERVAL).await;
            }
        }
    }
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::PollProvisioner,
        provisioner_pod_id = provisioner_pod_id.as_str();
        "runpod provision step completed"
    );

    terminate_provisioner_pod(
        &context,
        runpod_client,
        &mut operation,
        &mut workspace,
        &provisioner_pod_id,
    )
    .await?;

    mark_step(
        &context,
        &mut operation,
        workspace.id.as_str(),
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
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::CreateTemplate,
        template_id = template_id.as_str();
        "runpod provision step completed"
    );

    mark_step(
        &context,
        &mut operation,
        workspace.id.as_str(),
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
    runpod_workspace_mut(&mut workspace).resources.endpoint_id = Some(endpoint_id.clone());
    workspace.state = WorkspaceState::Ready;
    workspace = context.persist_workspace(workspace).await?;
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::CreateEndpoint,
        endpoint_id = endpoint_id.as_str();
        "runpod provision step completed"
    );
    Ok(workspace)
}

fn runpod_workspace(workspace: &Workspace) -> &RunpodRuntime {
    let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
    runtime
}

fn runpod_workspace_mut(workspace: &mut Workspace) -> &mut RunpodRuntime {
    let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
    runtime
}

async fn terminate_provisioner_pod(
    context: &WorkspaceRuntimeContext<'_>,
    runpod_client: &dyn RunpodRuntimeClient,
    operation: &mut LifecycleOperation,
    workspace: &mut Workspace,
    provisioner_pod_id: &str,
) -> Result<(), WorkspaceError> {
    mark_step(
        context,
        operation,
        workspace.id.as_str(),
        RunpodProvisionStep::TerminateProvisionerPod,
    )
    .await?;
    runpod_client
        .terminate_provisioner_pod(provisioner_pod_id)
        .await
        .map_err(super::runtime::map_provider_error)?;
    runpod_workspace_mut(workspace).resources.provisioner_pod_id = None;
    *workspace = context.persist_workspace(workspace.clone()).await?;
    log::info!(
        workspace_id = workspace.id.as_str(),
        operation_id = operation.operation_id.as_str(),
        step:? = RunpodProvisionStep::TerminateProvisionerPod,
        provisioner_pod_id = provisioner_pod_id;
        "runpod provision step completed"
    );
    Ok(())
}

fn provisioner_worker_unavailable_timed_out(
    provisioner_status_responded: bool,
    consecutive_unavailable_status_polls: usize,
    started_at: Instant,
    now: Instant,
) -> bool {
    if provisioner_status_responded {
        consecutive_unavailable_status_polls >= PROVISIONER_UNAVAILABLE_RETRY_LIMIT_AFTER_STARTUP
    } else {
        provisioner_startup_probe_expired(started_at, now)
    }
}

fn provisioner_startup_probe_expired(started_at: Instant, now: Instant) -> bool {
    now.duration_since(started_at) >= PROVISIONER_STARTUP_TIMEOUT
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleOperation, LifecycleOperationPayload, LifecycleProvisionPayload,
            },
            runpod::RunpodProvisionStep,
            runtime_contract::RuntimeCatalog,
            workflow_preset::WorkflowCatalog,
            workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
        },
        provider::errors::ProviderApiError,
        provider::runpod::runtime::RunpodRuntimeClient,
        provider::runpod::test_support::{
            runpod_client_with_failed_provisioner, runpod_client_with_failure,
            runpod_client_with_provisioner_status_sequence, runpod_client_with_state,
            runpod_client_with_transient_unavailable_provisioner, workspace_with_runpod,
            RunpodClientFailure,
        },
        runtime_catalog::{
            BundledRuntimeCatalogRepository, RuntimeCatalogError, RuntimeCatalogRepository,
        },
        workflow_catalog::{
            BundledWorkflowCatalogRepository, WorkflowCatalogError, WorkflowCatalogRepository,
        },
        workspace::{
            test_support::runtime_context_for_test, WorkspaceError, WorkspaceRuntimeContext,
        },
    };

    #[derive(Debug, Clone, Copy)]
    struct EmptyWorkflowCatalogRepository;

    impl WorkflowCatalogRepository for EmptyWorkflowCatalogRepository {
        fn get_workflow_catalog(&self) -> Result<WorkflowCatalog, WorkflowCatalogError> {
            Ok(WorkflowCatalog {
                workflow_presets: vec![],
            })
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct EmptyRuntimeCatalogRepository;

    impl RuntimeCatalogRepository for EmptyRuntimeCatalogRepository {
        fn get_runtime_contract_catalog(&self) -> Result<RuntimeCatalog, RuntimeCatalogError> {
            Ok(RuntimeCatalog { contracts: vec![] })
        }
    }

    async fn provision_with_bundled_catalogs(
        context: WorkspaceRuntimeContext<'_>,
        runpod_client: &dyn RunpodRuntimeClient,
        operation: LifecycleOperation,
        workspace: Workspace,
    ) -> Result<Workspace, WorkspaceError> {
        let workflow_catalog = BundledWorkflowCatalogRepository::new();
        let runtime_catalog = BundledRuntimeCatalogRepository::new();

        super::provision_workspace(
            context,
            runpod_client,
            &workflow_catalog,
            &runtime_catalog,
            operation,
            workspace,
        )
        .await
    }

    #[tokio::test]
    async fn provision_uses_injected_workflow_catalog() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_state();

        let error = super::provision_workspace(
            context,
            runpod_client.as_ref(),
            &EmptyWorkflowCatalogRepository,
            &EmptyRuntimeCatalogRepository,
            operation,
            workspace,
        )
        .await
        .expect_err("provision should use the injected empty workflow catalog");

        assert_eq!(
            error,
            WorkspaceError::InvalidState {
                message: "workflow reference was not found".to_string()
            }
        );
    }

    #[tokio::test]
    async fn provision_creates_all_runpod_resources_and_marks_workspace_ready() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_state();

        let provisioned = provision_with_bundled_catalogs(
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
                None,
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

            let error = provision_with_bundled_catalogs(
                context.clone(),
                runpod_client.as_ref(),
                operation,
                workspace,
            )
            .await
            .expect_err("provision should fail");

            assert_eq!(
                error,
                WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed {
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

        let error = provision_with_bundled_catalogs(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect_err("provision should fail");

        assert_eq!(
            error,
            WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed {
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
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);
    }

    #[tokio::test]
    async fn provision_terminates_provisioner_when_status_request_fails() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_failure(RunpodClientFailure::GetProvisionerStatus);

        let error = provision_with_bundled_catalogs(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect_err("provision should fail");

        assert_eq!(
            error,
            WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed {
                message: "get provisioner status failed".to_string(),
            })
        );
        let persisted = context
            .find_workspace_for_test("workspace-1")
            .await
            .expect("persisted workspace");
        let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
        assert_eq!(
            runtime.resources.network_volume_id,
            Some("volume".to_string())
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);
    }

    #[tokio::test]
    async fn provision_terminates_provisioner_when_status_poll_retry_limit_is_exhausted() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let unavailable = || {
            Err(
                super::super::errors::RunpodProviderError::ProvisionerWorkerUnavailable {
                    message: "provisioner worker is unavailable".to_string(),
                },
            )
        };
        let runpod_client = runpod_client_with_provisioner_status_sequence(vec![
            Ok(super::RunpodProvisionerStatus::Running),
            unavailable(),
            unavailable(),
            unavailable(),
        ]);

        let error = provision_with_bundled_catalogs(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect_err("provision should fail after provisioner status retry limit");

        assert_eq!(
            error,
            WorkspaceError::ProviderApiError(ProviderApiError::RequestFailed {
                message: "provisioner worker unavailable after startup retry limit".to_string(),
            })
        );
        let persisted = context
            .find_workspace_for_test("workspace-1")
            .await
            .expect("persisted workspace");
        let WorkspaceRuntime::Runpod(runtime) = persisted.runtime;
        assert_eq!(
            runtime.resources.network_volume_id,
            Some("volume".to_string())
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);
    }

    #[tokio::test]
    async fn provision_keeps_polling_when_provisioner_is_temporarily_unavailable() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_transient_unavailable_provisioner();

        let provisioned = provision_with_bundled_catalogs(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect("provision should tolerate transient provisioner startup unavailability");

        assert_eq!(provisioned.state, WorkspaceState::Ready);
    }

    #[tokio::test]
    async fn provision_keeps_polling_when_provisioner_is_unavailable_after_successful_status() {
        let context = runtime_context_for_test();
        let workspace = workspace_with_runpod("workspace-1", WorkspaceState::NotProvisioned);
        context.insert_workspace_for_test(workspace.clone()).await;
        let operation = context.create_operation_for_test("workspace-1").await;
        let runpod_client = runpod_client_with_provisioner_status_sequence(vec![
            Ok(super::RunpodProvisionerStatus::Running),
            Err(
                super::super::errors::RunpodProviderError::ProvisionerWorkerUnavailable {
                    message: "provisioner worker is unavailable".to_string(),
                },
            ),
            Ok(super::RunpodProvisionerStatus::Succeeded),
        ]);

        let provisioned = provision_with_bundled_catalogs(
            context.clone(),
            runpod_client.as_ref(),
            operation,
            workspace,
        )
        .await
        .expect("provision should tolerate transient provisioner unavailability after startup");

        assert_eq!(provisioned.state, WorkspaceState::Ready);
    }

    #[test]
    fn provisioner_startup_probe_expires_at_timeout() {
        let started_at = std::time::Instant::now();

        assert!(!super::provisioner_startup_probe_expired(
            started_at,
            started_at + super::PROVISIONER_STARTUP_TIMEOUT - std::time::Duration::from_secs(1),
        ));
        assert!(super::provisioner_startup_probe_expired(
            started_at,
            started_at + super::PROVISIONER_STARTUP_TIMEOUT,
        ));
    }

    #[test]
    fn provisioner_worker_unavailable_times_out_before_startup_or_after_retry_budget() {
        let started_at = std::time::Instant::now();
        let timeout_at = started_at + super::PROVISIONER_STARTUP_TIMEOUT;

        assert!(super::provisioner_worker_unavailable_timed_out(
            false, 0, started_at, timeout_at
        ));
        assert!(!super::provisioner_worker_unavailable_timed_out(
            true, 1, started_at, timeout_at
        ));
        assert!(!super::provisioner_worker_unavailable_timed_out(
            true, 2, started_at, timeout_at
        ));
        assert!(super::provisioner_worker_unavailable_timed_out(
            true, 3, started_at, timeout_at
        ));
    }
}
