use std::sync::Arc;

use crate::{
    diagnostics::{lifecycle_log_fields, lifecycle_state_label},
    domain::{
        lifecycle_operation::{LifecycleOperation, LifecycleOperationPayload},
        runpod::RunpodLifecycleOperationPayload,
        runpod::{RunpodPlacementOptions, RunpodPlacementPlan},
        runpod::{RunpodResources, RunpodRuntime},
        workflow_preset::WorkflowReference,
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    shared::BackgroundTaskSpawner,
    workspace_catalog::WorkspaceCatalogRepository,
};

use super::{
    catalogs::RunpodRuntimeCatalogServices,
    contracts::RunpodWorkflowResolver,
    errors::{
        invalid_runtime_state_message, lifecycle_operation_already_running, workspace_not_found,
        RunpodRuntimeError,
    },
    events::{RunpodRuntimeEvent, RunpodRuntimeEventSink},
    lifecycle::{
        helpers::{
            interrupted_state_for_resources, payload_with_app_interrupted_error,
            runpod_resources_are_empty,
        },
        runner::{
            LifecycleOperationRegistry, RunpodRuntimeLifecycleRunner,
            RunpodRuntimeLifecycleRunnerContext,
        },
    },
    provider::RunpodRuntimeClient,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRunpodWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset_id: String,
    pub placement: RunpodPlacementPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CreateWorkspaceServiceSpanFields {
    workspace_id: String,
    workflow_preset_id: String,
    datacenter_id: String,
    gpu_type_id: String,
    volume_size_gb: u64,
}

fn create_workspace_service_span_fields(
    request: &CreateRunpodWorkspaceRequest,
) -> CreateWorkspaceServiceSpanFields {
    CreateWorkspaceServiceSpanFields {
        workspace_id: request.workspace_id.clone(),
        workflow_preset_id: request.workflow_preset_id.clone(),
        datacenter_id: request.placement.data_center_id.clone(),
        gpu_type_id: request.placement.gpu_type_id.clone(),
        volume_size_gb: request.placement.volume_size_gb,
    }
}

pub struct RunpodRuntimeService<W, L>
where
    W: WorkspaceCatalogRepository,
    L: crate::lifecycle_journal::LifecycleJournalRepository,
{
    workspace_repository: W,
    lifecycle_journal: L,
    catalogs: RunpodRuntimeCatalogServices,
    runpod_client: Arc<dyn RunpodRuntimeClient>,
    lifecycle_operation_registry: LifecycleOperationRegistry,
    event_sink: Arc<dyn RunpodRuntimeEventSink>,
    task_spawner: Arc<dyn BackgroundTaskSpawner>,
    lifecycle_runner: Arc<dyn RunpodRuntimeLifecycleRunner<W, L>>,
}

impl<W, L> RunpodRuntimeService<W, L>
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: crate::lifecycle_journal::LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    pub(crate) fn new(
        workspace_repository: W,
        lifecycle_journal: L,
        catalogs: RunpodRuntimeCatalogServices,
        runpod_client: Arc<dyn RunpodRuntimeClient>,
        event_sink: Arc<dyn RunpodRuntimeEventSink>,
        task_spawner: Arc<dyn BackgroundTaskSpawner>,
        lifecycle_runner: Arc<dyn RunpodRuntimeLifecycleRunner<W, L>>,
    ) -> Self {
        Self {
            workspace_repository,
            lifecycle_journal,
            catalogs,
            runpod_client,
            lifecycle_operation_registry: LifecycleOperationRegistry::default(),
            event_sink,
            task_spawner,
            lifecycle_runner,
        }
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(
            service_operation = "create_runpod_workspace",
            workspace_id = %request.workspace_id,
            workflow_preset_id = %request.workflow_preset_id,
            datacenter_id = %request.placement.data_center_id,
            gpu_type_id = %request.placement.gpu_type_id,
            volume_size_gb = request.placement.volume_size_gb,
            request_metadata = tracing::field::debug(create_workspace_service_span_fields(&request))
        )
    )]
    pub async fn create_runpod_workspace(
        &self,
        request: CreateRunpodWorkspaceRequest,
    ) -> Result<Workspace, RunpodRuntimeError> {
        if request.workspace_id.trim().is_empty() {
            return Err(invalid_runtime_state_message("workspace id is required"));
        }

        let workflow_catalog = self
            .catalogs
            .workflow_catalog
            .get_workflow_catalog()
            .map_err(RunpodRuntimeError::from)?;
        let workflow =
            RunpodWorkflowResolver::resolve_latest(&workflow_catalog, &request.workflow_preset_id)
                .ok_or_else(|| invalid_runtime_state_message("workflow preset was not found"))?;
        if request.placement.volume_size_gb < workflow.required_volume_size_gb {
            return Err(invalid_runtime_state_message(
                "requested volume is smaller than the workflow requires",
            ));
        }

        let workspace = Workspace {
            id: request.workspace_id,
            workflow: WorkflowReference {
                id: workflow.id,
                version: workflow.version,
            },
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntime::Runpod(RunpodRuntime {
                placement: request.placement,
                resources: RunpodResources {
                    network_volume_id: None,
                    provisioner_pod_id: None,
                    endpoint_id: None,
                    template_id: None,
                },
            }),
        };

        let workspace = self
            .workspace_repository
            .insert_workspace(&workspace)
            .await
            .map_err(RunpodRuntimeError::from)?;

        self.event_sink.emit(RunpodRuntimeEvent::WorkspaceChanged {
            workspace_id: workspace.id.clone(),
            workspace: Box::new(workspace.clone()),
        });

        Ok(workspace)
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "get_runpod_placement_options")
    )]
    pub async fn get_runpod_placement_options(
        &self,
    ) -> Result<RunpodPlacementOptions, RunpodRuntimeError> {
        self.runpod_client.placement_options().await
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "provision_workspace", workspace_id = %workspace_id)
    )]
    pub async fn provision_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<ProvisionWorkspaceResponse, RunpodRuntimeError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        if workspace.state != WorkspaceState::NotProvisioned {
            return Err(invalid_runtime_state_message(
                "workspace is not ready to provision",
            ));
        }
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if !runpod_resources_are_empty(&runtime.resources) {
            return Err(invalid_runtime_state_message(
                "workspace already has runpod resources",
            ));
        }

        let payload =
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: None,
            });
        let (workspace, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;
        self.lifecycle_runner.spawn_provision(
            self.lifecycle_runner_context(),
            operation.operation_id.clone(),
        );

        Ok(ProvisionWorkspaceResponse {
            workspace,
            operation,
        })
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "cleanup_workspace", workspace_id = %workspace_id)
    )]
    pub async fn cleanup_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<CleanupWorkspaceResponse, RunpodRuntimeError> {
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
            step: None,
        });
        let (workspace, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;
        self.lifecycle_runner.spawn_cleanup(
            self.lifecycle_runner_context(),
            operation.operation_id.clone(),
        );

        Ok(CleanupWorkspaceResponse {
            workspace,
            operation,
        })
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "delete_workspace", workspace_id = %workspace_id)
    )]
    pub async fn delete_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<DeleteWorkspaceResponse, RunpodRuntimeError> {
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: None,
        });
        let (workspace, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;

        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        if runpod_resources_are_empty(&runtime.resources) {
            let completed_operation = super::lifecycle::delete::run_once(
                &operation.operation_id,
                &self.workspace_repository,
                &self.lifecycle_journal,
                self.runpod_client.as_ref(),
                &self.event_sink,
            )
            .await?
            .ok_or_else(|| {
                invalid_runtime_state_message("delete lifecycle operation was not found")
            })?;

            return Ok(DeleteWorkspaceResponse {
                workspace_id: workspace_id.to_string(),
                operation: completed_operation,
            });
        }

        self.lifecycle_runner.spawn_delete(
            self.lifecycle_runner_context(),
            operation.operation_id.clone(),
        );

        Ok(DeleteWorkspaceResponse {
            workspace_id: workspace_id.to_string(),
            operation,
        })
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "get_running_lifecycle_operations")
    )]
    pub async fn get_running_lifecycle_operations(
        &self,
    ) -> Result<Vec<LifecycleOperation>, RunpodRuntimeError> {
        self.lifecycle_journal
            .list_running()
            .await
            .map_err(super::errors::invalid_runtime_state_error)
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "get_latest_lifecycle_operation", workspace_id = %workspace_id)
    )]
    pub async fn get_latest_lifecycle_operation(
        &self,
        workspace_id: &str,
    ) -> Result<Option<LifecycleOperation>, RunpodRuntimeError> {
        let workspace_id = workspace_id.to_string();
        self.lifecycle_journal
            .latest_for_workspace(&workspace_id)
            .await
            .map_err(super::errors::invalid_runtime_state_error)
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "find_workspace", workspace_id = %workspace_id)
    )]
    pub async fn find_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<Workspace>, RunpodRuntimeError> {
        self.workspace_repository
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(RunpodRuntimeError::from)
    }

    #[tracing::instrument(
        name = "runpod_runtime_service",
        skip_all,
        fields(service_operation = "mark_running_operations_stale")
    )]
    pub async fn mark_running_operations_stale(&self) -> Result<(), RunpodRuntimeError> {
        let operations = self.get_running_lifecycle_operations().await?;

        for operation in operations {
            let payload = payload_with_app_interrupted_error(&operation.payload);

            let workspace = match self.find_workspace(&operation.workspace_id).await? {
                Some(mut workspace) => {
                    let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
                    workspace.state = interrupted_state_for_resources(&runtime.resources);
                    Some(
                        self.workspace_repository
                            .update_workspace(&workspace)
                            .await
                            .map_err(RunpodRuntimeError::from)?,
                    )
                }
                None => None,
            };

            let stale_operation = self
                .lifecycle_journal
                .mark_state(
                    &operation.operation_id,
                    crate::domain::lifecycle_operation::LifecycleOperationState::Stale,
                    &payload,
                )
                .await
                .map_err(super::errors::invalid_runtime_state_error)?;
            let fields = lifecycle_log_fields(&stale_operation.payload);
            tracing::info!(
                workspace_id = %stale_operation.workspace_id,
                operation_id = %stale_operation.operation_id,
                operation_kind = fields.operation_kind,
                state = lifecycle_state_label(stale_operation.state),
                step = fields.step.unwrap_or("none"),
                "lifecycle operation stale"
            );

            self.event_sink
                .emit(RunpodRuntimeEvent::LifecycleOperationChanged {
                    workspace_id: stale_operation.workspace_id.clone(),
                    operation_id: stale_operation.operation_id.clone(),
                    diagnostic_id: None,
                    operation: stale_operation,
                });
            if let Some(workspace) = workspace {
                self.event_sink.emit(RunpodRuntimeEvent::WorkspaceChanged {
                    workspace_id: workspace.id.clone(),
                    workspace: Box::new(workspace),
                });
            }
        }

        Ok(())
    }

    #[tracing::instrument(
        name = "runpod_lifecycle_start",
        skip_all,
        fields(workspace_id = %workspace_id, operation_kind = tracing::field::Empty)
    )]
    async fn start_lifecycle_operation(
        &self,
        workspace_id: &str,
        payload: &LifecycleOperationPayload,
    ) -> Result<(Workspace, LifecycleOperation), RunpodRuntimeError> {
        let fields = lifecycle_log_fields(payload);
        tracing::Span::current().record("operation_kind", fields.operation_kind);

        let workspace = self.load_workspace_required(workspace_id).await?;
        let workspace_id = workspace.id.clone();

        if self
            .lifecycle_journal
            .find_running_by_workspace(&workspace_id)
            .await
            .map_err(super::errors::invalid_runtime_state_error)?
            .is_some()
        {
            return Err(lifecycle_operation_already_running(workspace_id));
        }

        let operation = self
            .lifecycle_journal
            .create_operation(&workspace_id, payload)
            .await
            .map_err(super::errors::invalid_runtime_state_error)?;
        let fields = lifecycle_log_fields(&operation.payload);
        tracing::info!(
            workspace_id = %operation.workspace_id,
            operation_id = %operation.operation_id,
            operation_kind = fields.operation_kind,
            state = lifecycle_state_label(operation.state),
            step = fields.step.unwrap_or("none"),
            "lifecycle operation started"
        );

        self.event_sink
            .emit(RunpodRuntimeEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                diagnostic_id: None,
                operation: operation.clone(),
            });

        Ok((workspace, operation))
    }

    async fn load_workspace_required(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, RunpodRuntimeError> {
        self.find_workspace(workspace_id)
            .await?
            .ok_or_else(|| workspace_not_found(workspace_id))
    }

    pub(crate) fn lifecycle_runner_context(&self) -> RunpodRuntimeLifecycleRunnerContext<W, L> {
        RunpodRuntimeLifecycleRunnerContext {
            workspace_repository: self.workspace_repository.clone(),
            lifecycle_journal: self.lifecycle_journal.clone(),
            catalogs: self.catalogs.clone(),
            runpod_client: self.runpod_client.clone(),
            lifecycle_operation_registry: self.lifecycle_operation_registry.clone(),
            event_sink: self.event_sink.clone(),
            task_spawner: self.task_spawner.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionWorkspaceResponse {
    pub workspace: Workspace,
    pub operation: LifecycleOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupWorkspaceResponse {
    pub workspace: Workspace,
    pub operation: LifecycleOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteWorkspaceResponse {
    pub workspace_id: String,
    pub operation: LifecycleOperation,
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::{LifecycleOperationPayload, LifecycleOperationState},
            runpod::{RunpodLifecycleOperationPayload, RunpodRuntime},
            workspace::{WorkspaceRuntime, WorkspaceState},
        },
        lifecycle_journal::LifecycleJournalRepository,
        runpod_runtime::{
            lifecycle::helpers::runpod_resources_are_empty,
            provider::RunpodProvisionerStatus,
            test_support::{
                block_on, draft_create_request, placement_options, service_with_state,
                service_with_state_and_workspace_repository, service_without_lifecycle_spawning,
                InMemoryWorkspaceRepository, ManualLifecycleRunnerExt, RunpodClientState,
                WorkspaceRepositoryState,
            },
        },
        shared::ApiError,
        workspace_catalog::WorkspaceCatalogRepository,
    };
    use std::sync::{Arc, Mutex};

    fn provisioner_unavailable() -> crate::runpod_runtime::errors::RunpodRuntimeError {
        crate::runpod_runtime::errors::RunpodRuntimeError::ProvisionerWorkerUnavailable {
            message: "provisioner worker is unavailable".to_string(),
        }
    }

    fn runpod_api_runtime_error() -> crate::runpod_runtime::errors::RunpodRuntimeError {
        crate::runpod_runtime::errors::RunpodRuntimeError::RunpodApiError(ApiError::RequestFailed {
            message: "RunPod API request failed".to_string(),
        })
    }

    #[test]
    fn create_workspace_service_span_fields_include_only_safe_context() {
        let request = draft_create_request("workspace-1");

        let fields = super::create_workspace_service_span_fields(&request);

        assert_eq!(fields.workspace_id, "workspace-1");
        assert_eq!(fields.workflow_preset_id, request.workflow_preset_id);
        assert_eq!(fields.datacenter_id, request.placement.data_center_id);
        assert_eq!(fields.gpu_type_id, request.placement.gpu_type_id);
        assert_eq!(fields.volume_size_gb, request.placement.volume_size_gb);
    }

    #[test]
    fn create_runpod_workspace_persists_not_provisioned_workspace_without_client_calls() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_with_state(state.clone());

        let workspace =
            block_on(service.create_runpod_workspace(draft_create_request("workspace-1")))
                .expect("workspace should be created");

        assert_eq!(workspace.id, "workspace-1");
        assert_eq!(workspace.workflow.id, "comfyui-hidream-o1-dev");
        assert_eq!(workspace.workflow.version, "1.0.0");
        assert_eq!(workspace.state, WorkspaceState::NotProvisioned);
        let WorkspaceRuntime::Runpod(RunpodRuntime {
            placement,
            resources,
        }) = &workspace.runtime;
        assert_eq!(placement.data_center_id, "dc");
        assert_eq!(placement.gpu_type_id, "gpu");
        assert!(runpod_resources_are_empty(resources));
        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());

        let persisted = block_on(
            service
                .workspace_repository
                .find_workspace_by_id("workspace-1"),
        )
        .expect("repository read should succeed")
        .expect("workspace should be persisted");
        assert_eq!(persisted, workspace);
    }

    #[test]
    fn create_runpod_workspace_rejects_missing_workflow_preset_without_persisting_or_provider_calls(
    ) {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_with_state(state.clone());
        let mut request = draft_create_request("workspace-1");
        request.workflow_preset_id = "missing-preset".to_string();

        let error =
            block_on(service.create_runpod_workspace(request)).expect_err("request should fail");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::InvalidRuntimeState { .. }
        ));
        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());
        let persisted = block_on(
            service
                .workspace_repository
                .find_workspace_by_id("workspace-1"),
        )
        .expect("repository read should succeed");
        assert_eq!(persisted, None);
    }

    #[test]
    fn create_runpod_workspace_rejects_volume_smaller_than_workflow_requires_without_persisting() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_with_state(state.clone());
        let mut request = draft_create_request("workspace-1");
        request.placement.volume_size_gb = 1;

        let error =
            block_on(service.create_runpod_workspace(request)).expect_err("request should fail");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::InvalidRuntimeState { .. }
        ));
        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());
        let persisted = block_on(
            service
                .workspace_repository
                .find_workspace_by_id("workspace-1"),
        )
        .expect("repository read should succeed");
        assert_eq!(persisted, None);
    }

    #[test]
    fn get_runpod_placement_options_returns_runpod_options() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_with_state(state.clone());

        let options = block_on(service.get_runpod_placement_options())
            .expect("placement options should resolve");

        assert_eq!(options, placement_options());
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec!["placement_options"]
        );
    }

    #[tokio::test]
    async fn provision_workspace_creates_running_operation_and_keeps_workspace_state_unchanged() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        let workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");

        let response = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start");

        assert_eq!(response.workspace, workspace);
        assert_eq!(response.operation.workspace_id, "workspace-1");
        assert_eq!(response.operation.state, LifecycleOperationState::Running);
        assert_eq!(
            response.operation.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: None,
            })
        );
        let persisted = service
            .workspace_repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("repository read should succeed")
            .expect("workspace should exist");
        assert_eq!(persisted.state, WorkspaceState::NotProvisioned);
    }

    #[tokio::test]
    async fn provision_workspace_rejects_when_running_operation_exists() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        service
            .provision_workspace("workspace-1")
            .await
            .expect("first provision should start");

        let error = service
            .provision_workspace("workspace-1")
            .await
            .expect_err("second provision should be rejected");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::LifecycleOperationAlreadyRunning { .. }
        ));
    }

    #[tokio::test]
    async fn provision_workspace_rejects_ready_workspace_without_side_effects() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_without_lifecycle_spawning(state.clone());
        let mut workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        workspace.state = WorkspaceState::Ready;
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace update should succeed");

        let error = service
            .provision_workspace("workspace-1")
            .await
            .expect_err("ready workspace should not start provision");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::InvalidRuntimeState { .. }
        ));
        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());
        assert_eq!(
            service
                .get_running_lifecycle_operations()
                .await
                .expect("operation read should succeed"),
            Vec::new()
        );
    }

    #[tokio::test]
    async fn provision_workspace_rejects_resource_bearing_workspace_without_side_effects() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_without_lifecycle_spawning(state.clone());
        let mut workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.network_volume_id = Some("existing-volume".to_string());
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace update should succeed");

        let error = service
            .provision_workspace("workspace-1")
            .await
            .expect_err("resource-bearing workspace should not start provision");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::InvalidRuntimeState { .. }
        ));
        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());
        assert_eq!(
            service
                .get_running_lifecycle_operations()
                .await
                .expect("operation read should succeed"),
            Vec::new()
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_steps_updates_resources_and_completes_workspace() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![
                RunpodProvisionerStatus::Running,
                RunpodProvisionerStatus::Succeeded,
            ],
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should complete");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Ready);
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(
            runtime.resources.network_volume_id.as_deref(),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.endpoint_id.as_deref(), Some("endpoint"));
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template"));

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Completed);
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "get_provisioner_status",
                "terminate_provisioner_pod",
                "create_serverless_template",
                "create_serverless_endpoint",
            ]
        );
        let state = state.lock().expect("state lock");
        assert_eq!(
            state.provisioner_image_refs,
            vec![
                "ghcr.io/p-shapov/luma-forge/provisioner-worker@sha256:e890fabcd11d95bab36d2495c6b49d802ad72ab7350ecf5c3595d22b1fb66089"
            ]
        );
        assert_eq!(
            state.endpoint_image_refs,
            vec![
                "ghcr.io/p-shapov/luma-forge/runpod-endpoint-worker@sha256:c7253ac8abbca0c4d849110132c327595ff224ab953eeb93462f16f52f74f3a1"
            ]
        );
        assert_ne!(state.provisioner_image_refs, vec!["provisioner"]);
        assert_ne!(
            state.endpoint_image_refs,
            vec!["runpod-endpoint-comfyui-hidream-o1-dev"]
        );
    }

    #[tokio::test]
    async fn provision_runner_failure_preserves_resources_and_sets_cleanup_required() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            start_provisioner_pod_error: Some(provisioner_unavailable()),
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state);
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(
            runtime.resources.network_volume_id.as_deref(),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::StartProvisionerPod),
            })
        );
    }

    #[tokio::test]
    async fn provision_runner_deletes_template_when_endpoint_creation_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            create_serverless_endpoint_error: Some(runpod_api_runtime_error()),
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id, None);
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "terminate_provisioner_pod",
                "create_serverless_template",
                "create_serverless_endpoint",
                "delete_template",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_reports_create_template_when_template_creation_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            create_serverless_template_error: Some(runpod_api_runtime_error()),
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id, None);

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::CreateTemplate),
            })
        );
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "terminate_provisioner_pod",
                "create_serverless_template",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_preserves_template_id_when_endpoint_creation_and_template_delete_fail(
    ) {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            create_serverless_endpoint_error: Some(runpod_api_runtime_error()),
            delete_template_error: Some(runpod_api_runtime_error()),
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template"));

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::CreateEndpoint),
            })
        );
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "terminate_provisioner_pod",
                "create_serverless_template",
                "create_serverless_endpoint",
                "delete_template",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_failed_status_terminates_provisioner_without_endpoint() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Failed],
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("provision runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(
            runtime.resources.network_volume_id.as_deref(),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner_pod_id, None);
        assert_eq!(runtime.resources.endpoint_id, None);

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::TerminateProvisionerPod),
            })
        );
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "terminate_provisioner_pod",
            ]
        );
    }

    #[tokio::test]
    async fn provision_workspace_spawned_runner_executes_runpod_client_flow() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            ..RunpodClientState::default()
        }));
        let service = service_with_state(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");

        service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start");

        let mut latest = None;
        for _ in 0..20 {
            latest = service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("operation read should succeed");
            if latest
                .as_ref()
                .is_some_and(|operation| operation.state == LifecycleOperationState::Completed)
            {
                break;
            }
            tokio::task::yield_now().await;
        }

        assert_eq!(
            latest.expect("operation should exist").state,
            LifecycleOperationState::Completed
        );
        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace read should succeed")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Ready);
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "get_provisioner_status",
                "terminate_provisioner_pod",
                "create_serverless_template",
                "create_serverless_endpoint",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_failed_when_workspace_missing_after_operation_created() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;
        service
            .workspace_repository
            .delete_workspace("workspace-1")
            .await
            .expect("workspace should be deleted");

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("runner should terminalize operation");

        assert!(service
            .get_running_lifecycle_operations()
            .await
            .expect("operations should load")
            .is_empty());
        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::CreateNetworkVolume),
            })
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_failed_when_workflow_reference_does_not_resolve() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;
        let mut workspace = service
            .workspace_repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        workspace.workflow.version = "missing-revision".to_string();
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace should update");

        service
            .run_provision_once_for_test(&operation_id)
            .await
            .expect("runner should terminalize operation");

        assert_eq!(state.lock().expect("state lock").calls, Vec::<&str>::new());
        assert!(service
            .get_running_lifecycle_operations()
            .await
            .expect("operations should load")
            .is_empty());
        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: Some(crate::domain::runpod::RunpodProvisionStep::CreateNetworkVolume),
            })
        );
        let workspace = service
            .workspace_repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
    }

    #[tokio::test]
    async fn cleanup_workspace_creates_cleanup_operation_without_changing_workspace_state() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        let workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");

        let response = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start");

        assert_eq!(response.workspace, workspace);
        assert_eq!(
            response.operation.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
                step: None,
            })
        );
        let persisted = service
            .workspace_repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("repository read should succeed")
            .expect("workspace should exist");
        assert_eq!(persisted.state, WorkspaceState::NotProvisioned);
    }

    #[tokio::test]
    async fn cleanup_runner_marks_failed_when_workspace_missing_after_operation_created() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation_id = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start")
            .operation
            .operation_id;
        service
            .workspace_repository
            .delete_workspace("workspace-1")
            .await
            .expect("workspace should be deleted");

        service
            .run_cleanup_once_for_test(&operation_id)
            .await
            .expect("runner should terminalize operation");

        assert!(service
            .get_running_lifecycle_operations()
            .await
            .expect("operations should load")
            .is_empty());
        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
                step: Some(crate::domain::runpod::RunpodCleanupStep::DeleteEndpoint),
            })
        );
    }

    #[tokio::test]
    async fn cleanup_runner_preserves_endpoint_id_when_endpoint_cleanup_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let provision_operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;
        service
            .run_provision_once_for_test(&provision_operation_id)
            .await
            .expect("provision should complete");

        state
            .lock()
            .expect("state lock")
            .delete_serverless_endpoint_error = Some(runpod_api_runtime_error());
        let cleanup_operation_id = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start")
            .operation
            .operation_id;
        service
            .run_cleanup_once_for_test(&cleanup_operation_id)
            .await
            .expect("cleanup runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(runtime.resources.endpoint_id.as_deref(), Some("endpoint"));
    }

    #[tokio::test]
    async fn cleanup_runner_reports_delete_template_when_template_cleanup_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let provision_operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;
        service
            .run_provision_once_for_test(&provision_operation_id)
            .await
            .expect("provision should complete");

        state.lock().expect("state lock").delete_template_error = Some(runpod_api_runtime_error());
        let cleanup_operation_id = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start")
            .operation
            .operation_id;
        service
            .run_cleanup_once_for_test(&cleanup_operation_id)
            .await
            .expect("cleanup runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template"));

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Cleanup {
                step: Some(crate::domain::runpod::RunpodCleanupStep::DeleteTemplate),
            })
        );
    }

    #[tokio::test]
    async fn delete_workspace_without_resources_completes_and_deletes_immediately() {
        let service = service_with_state(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");

        let response = service
            .delete_workspace("workspace-1")
            .await
            .expect("delete should start");

        assert_eq!(response.workspace_id, "workspace-1");
        assert_eq!(response.operation.state, LifecycleOperationState::Completed);
        assert_eq!(
            response.operation.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
                step: Some(crate::domain::runpod::RunpodDeleteStep::DeleteLocalWorkspace),
            })
        );
        assert!(service
            .workspace_repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("repository read should succeed")
            .is_none());
        assert_eq!(
            service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("operation read should succeed"),
            None
        );
    }

    #[tokio::test]
    async fn delete_workspace_without_resources_preserves_lifecycle_row_when_workspace_delete_fails(
    ) {
        let workspace_state = Arc::new(Mutex::new(WorkspaceRepositoryState::default()));
        let service = service_with_state_and_workspace_repository(
            Arc::new(Mutex::new(RunpodClientState::default())),
            InMemoryWorkspaceRepository::with_state(workspace_state.clone()),
        );
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        workspace_state
            .lock()
            .expect("workspace state lock should succeed")
            .delete_workspace_error = Some(
            crate::workspace_catalog::WorkspaceCatalogError::StorageUnavailable {
                message: "query failed".to_string(),
            },
        );

        let error = service
            .delete_workspace("workspace-1")
            .await
            .expect_err("delete should fail when workspace delete fails");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::WorkspaceCatalogInvalid(_)
        ));
        assert!(service
            .find_workspace("workspace-1")
            .await
            .expect("workspace lookup should succeed")
            .is_some());
        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation lookup should succeed")
            .expect("operation should remain for diagnosis");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
                step: Some(crate::domain::runpod::RunpodDeleteStep::DeleteLocalWorkspace),
            })
        );
    }

    #[tokio::test]
    async fn delete_runner_completes_when_workspace_missing_after_operation_created() {
        let service = service_with_state(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: None,
        });
        let operation_id = service
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("delete operation should be created")
            .operation_id;
        service
            .workspace_repository
            .delete_workspace("workspace-1")
            .await
            .expect("workspace should be deleted");

        service
            .run_delete_once_for_test(&operation_id)
            .await
            .expect("runner should terminalize operation");

        assert!(service
            .get_running_lifecycle_operations()
            .await
            .expect("operations should load")
            .is_empty());
        assert_eq!(
            service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("operation should load"),
            None
        );
    }

    #[tokio::test]
    async fn delete_runner_success_deletes_workspace_and_lifecycle_rows() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let service = service_with_state(state.clone());
        let mut workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.network_volume_id = Some("volume".to_string());
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace should update");
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: None,
        });
        let operation_id = service
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("delete operation should be created")
            .operation_id;

        service
            .run_delete_once_for_test(&operation_id)
            .await
            .expect("delete runner should complete");

        assert!(service
            .find_workspace("workspace-1")
            .await
            .expect("workspace lookup should succeed")
            .is_none());
        assert_eq!(
            service
                .get_latest_lifecycle_operation("workspace-1")
                .await
                .expect("operation lookup should succeed"),
            None
        );
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec!["delete_network_volume"]
        );
    }

    #[tokio::test]
    async fn delete_runner_reports_delete_template_when_template_delete_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState {
            provisioner_status_results: vec![RunpodProvisionerStatus::Succeeded],
            ..RunpodClientState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let provision_operation_id = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation
            .operation_id;
        service
            .run_provision_once_for_test(&provision_operation_id)
            .await
            .expect("provision should complete");

        state.lock().expect("state lock").delete_template_error = Some(runpod_api_runtime_error());
        let delete_operation_id = service
            .delete_workspace("workspace-1")
            .await
            .expect("delete should start")
            .operation
            .operation_id;
        service
            .run_delete_once_for_test(&delete_operation_id)
            .await
            .expect("delete runner should record failure");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        let WorkspaceRuntime::Runpod(runtime) = &workspace.runtime;
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template"));

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
                step: Some(crate::domain::runpod::RunpodDeleteStep::DeleteTemplate),
            })
        );
    }

    #[tokio::test]
    async fn delete_runner_preserves_lifecycle_row_when_workspace_delete_fails() {
        let state = Arc::new(Mutex::new(RunpodClientState::default()));
        let workspace_state = Arc::new(Mutex::new(WorkspaceRepositoryState::default()));
        let service = service_with_state_and_workspace_repository(
            state,
            InMemoryWorkspaceRepository::with_state(workspace_state.clone()),
        );
        let mut workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.network_volume_id = Some("volume".to_string());
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace should update");
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: None,
        });
        let operation_id = service
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("delete operation should be created")
            .operation_id;
        workspace_state
            .lock()
            .expect("workspace state lock should succeed")
            .delete_workspace_error = Some(
            crate::workspace_catalog::WorkspaceCatalogError::StorageUnavailable {
                message: "query failed".to_string(),
            },
        );

        let error = service
            .run_delete_once_for_test(&operation_id)
            .await
            .expect_err("delete runner should fail when workspace delete fails");

        assert!(matches!(
            error,
            crate::runpod_runtime::errors::RunpodRuntimeError::WorkspaceCatalogInvalid(_)
        ));
        assert!(service
            .find_workspace("workspace-1")
            .await
            .expect("workspace lookup should succeed")
            .is_some());
        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation lookup should succeed")
            .expect("operation should remain for diagnosis");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
                step: Some(crate::domain::runpod::RunpodDeleteStep::DeleteLocalWorkspace),
            })
        );
    }

    #[tokio::test]
    async fn get_running_lifecycle_operations_returns_started_operations() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start")
            .operation;

        let operations = service
            .get_running_lifecycle_operations()
            .await
            .expect("running operations should load");

        assert_eq!(operations, vec![operation]);
    }

    #[tokio::test]
    async fn get_latest_lifecycle_operation_returns_latest_for_workspace() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let first = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation;
        service
            .lifecycle_journal
            .mark_state(
                &first.operation_id,
                LifecycleOperationState::Completed,
                &first.payload,
            )
            .await
            .expect("operation should complete");
        let latest = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start")
            .operation;

        let operation = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("latest operation should load")
            .expect("latest operation should exist");

        assert_eq!(operation.operation_id, latest.operation_id);
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_workspace_invalid_when_no_resources_exist() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation;

        service
            .mark_running_operations_stale()
            .await
            .expect("running operations should be marked stale");

        let stale = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(
            stale.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: None,
            })
        );
        assert_eq!(stale.operation_id, operation.operation_id);
        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::Invalid);
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_workspace_cleanup_required_when_resources_exist() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        let mut workspace = service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::Runpod(runtime) = &mut workspace.runtime;
        runtime.resources.network_volume_id = Some("volume-1".to_string());
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace update should persist");
        service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start");

        service
            .mark_running_operations_stale()
            .await
            .expect("running operations should be marked stale");

        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(workspace.state, WorkspaceState::CleanupRequired);
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_delete_stale_when_workspace_is_missing() {
        let service = service_with_state(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let payload = LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
            step: None,
        });
        let operation = service
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("delete operation should be created");
        service
            .workspace_repository
            .delete_workspace("workspace-1")
            .await
            .expect("workspace should be removed");

        service
            .mark_running_operations_stale()
            .await
            .expect("missing delete workspace should still mark operation stale");

        let stale = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(stale.operation_id, operation.operation_id);
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(
            stale.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Delete {
                step: None,
            })
        );
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_provision_stale_when_workspace_is_missing() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(RunpodClientState::default())));
        service
            .create_runpod_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let operation = service
            .provision_workspace("workspace-1")
            .await
            .expect("provision should start")
            .operation;
        service
            .workspace_repository
            .delete_workspace("workspace-1")
            .await
            .expect("workspace should be removed");

        service
            .mark_running_operations_stale()
            .await
            .expect("missing provision workspace should still mark operation stale");

        assert!(service
            .get_running_lifecycle_operations()
            .await
            .expect("running operations should load")
            .is_empty());
        let stale = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation should load")
            .expect("operation should exist");
        assert_eq!(stale.operation_id, operation.operation_id);
        assert_eq!(stale.state, LifecycleOperationState::Stale);
        assert_eq!(
            stale.payload,
            LifecycleOperationPayload::Runpod(RunpodLifecycleOperationPayload::Provision {
                step: None,
            })
        );
    }
}
