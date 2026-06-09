use std::{sync::Arc, time::Duration};

use crate::{
    domain::{
        lifecycle_operation::{
            LifecycleOperation, LifecycleOperationPayload,
            ProvisionedRemoteLifecycleOperationPayload,
        },
        placement::{RemotePlacementOptions, RemotePlacementPlan},
        provider::GpuCloudProviderId,
        provisioned_remote::{
            ProvisionedRemoteDeleteStep, ProvisionedRemoteResources, ProvisionedRemoteRuntime,
        },
        workflow_preset::WorkflowPreset,
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

use super::{
    errors::ProvisionedRemoteError,
    events::{NoopProvisionedRemoteEventSink, ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
    lifecycle::{
        background::spawn_lifecycle_task,
        coordination::LifecycleOperationRegistry,
        helpers::{
            interrupted_state_for_resources, map_lifecycle_journal_error,
            payload_with_app_interrupted_error,
        },
    },
    registry::ProvisionedRemoteProviderRegistry,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProvisionedRemoteWorkspaceRequest {
    pub workspace_id: String,
    pub workflow_preset: WorkflowPreset,
    pub remote_placement: RemotePlacementPlan,
}

pub struct ProvisionedRemoteService<W, L>
where
    W: WorkspaceCatalogRepository,
    L: crate::lifecycle_journal::LifecycleJournalRepository,
{
    workspace_repository: W,
    lifecycle_journal: L,
    provider_registry: ProvisionedRemoteProviderRegistry,
    lifecycle_operation_registry: LifecycleOperationRegistry,
    event_sink: Arc<dyn ProvisionedRemoteEventSink>,
    provisioner_poll_interval: Duration,
    #[cfg(test)]
    spawn_lifecycle_runners: bool,
}

impl<W, L> ProvisionedRemoteService<W, L>
where
    W: WorkspaceCatalogRepository + Clone + Send + Sync + 'static,
    L: crate::lifecycle_journal::LifecycleJournalRepository + Clone + Send + Sync + 'static,
{
    pub fn new(
        workspace_repository: W,
        lifecycle_journal: L,
        provider_registry: ProvisionedRemoteProviderRegistry,
    ) -> Self {
        Self {
            workspace_repository,
            lifecycle_journal,
            provider_registry,
            lifecycle_operation_registry: LifecycleOperationRegistry::default(),
            event_sink: Arc::new(NoopProvisionedRemoteEventSink),
            provisioner_poll_interval: Duration::from_secs(5),
            #[cfg(test)]
            spawn_lifecycle_runners: true,
        }
    }

    pub fn with_event_sink(mut self, event_sink: Arc<dyn ProvisionedRemoteEventSink>) -> Self {
        self.event_sink = event_sink;
        self
    }

    #[cfg(test)]
    pub fn without_lifecycle_spawning(mut self) -> Self {
        self.spawn_lifecycle_runners = false;
        self
    }

    #[cfg(test)]
    pub fn provisioner_poll_interval(&self) -> Duration {
        self.provisioner_poll_interval
    }

    pub async fn create_workspace(
        &self,
        request: CreateProvisionedRemoteWorkspaceRequest,
    ) -> Result<Workspace, ProvisionedRemoteError> {
        if request.workspace_id.trim().is_empty() {
            return Err(ProvisionedRemoteError::InvalidRuntimeState);
        }

        request
            .workflow_preset
            .remote_runtime_requirements
            .resolve_provider_requirements(request.remote_placement.gpu_cloud_provider_id)
            .ok_or(ProvisionedRemoteError::InvalidRuntimeState)?;

        let workspace = Workspace {
            id: request.workspace_id,
            workflow_preset: request.workflow_preset,
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
                placement: request.remote_placement,
                resources: ProvisionedRemoteResources {
                    volume: None,
                    provisioner: None,
                    endpoint: None,
                },
            }),
        };

        let workspace = self
            .workspace_repository
            .insert_workspace(&workspace)
            .await
            .map_err(map_workspace_catalog_error)?;

        self.event_sink
            .emit(ProvisionedRemoteEvent::WorkspaceChanged {
                workspace_id: workspace.id.clone(),
                workspace: workspace.clone(),
            });

        Ok(workspace)
    }

    pub async fn get_provider_placement_options(
        &self,
        provider_id: GpuCloudProviderId,
    ) -> Result<RemotePlacementOptions, ProvisionedRemoteError> {
        self.provider_registry
            .for_provider(provider_id)?
            .get_provider_placement_options()
            .await
    }

    pub async fn provision_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<ProvisionWorkspaceResponse, ProvisionedRemoteError> {
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Provision {
                step: None,
                error: None,
            },
        );
        let (workspace, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;
        self.spawn_provision_runner(&operation);

        Ok(ProvisionWorkspaceResponse {
            workspace,
            operation,
        })
    }

    pub async fn cleanup_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<CleanupWorkspaceResponse, ProvisionedRemoteError> {
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                step: None,
                error: None,
            },
        );
        let (workspace, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;
        self.spawn_cleanup_runner(&operation);

        Ok(CleanupWorkspaceResponse {
            workspace,
            operation,
        })
    }

    pub async fn delete_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<DeleteWorkspaceResponse, ProvisionedRemoteError> {
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: None,
                error: None,
            },
        );
        let (_, operation) = self
            .start_lifecycle_operation(workspace_id, &payload)
            .await?;

        let workspace = self.load_workspace_required(workspace_id).await?;
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        if runtime.resources.is_empty() {
            let payload = LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Delete {
                    step: Some(ProvisionedRemoteDeleteStep::DeleteLocalWorkspace),
                    error: None,
                },
            );
            let completed_operation = self
                .lifecycle_journal
                .mark_state(
                    &operation.operation_id,
                    crate::domain::lifecycle_operation::LifecycleOperationState::Completed,
                    &payload,
                )
                .await
                .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;

            self.event_sink
                .emit(ProvisionedRemoteEvent::LifecycleOperationChanged {
                    workspace_id: completed_operation.workspace_id.clone(),
                    operation_id: completed_operation.operation_id.clone(),
                    operation: completed_operation.clone(),
                });

            self.workspace_repository
                .delete_workspace(&workspace.id)
                .await
                .map_err(map_workspace_catalog_error)?;
            self.lifecycle_journal
                .delete_for_workspace(&workspace.id)
                .await
                .map_err(|error| map_lifecycle_journal_error(error, &workspace.id))?;
            self.event_sink
                .emit(ProvisionedRemoteEvent::WorkspaceDeleted {
                    workspace_id: workspace.id.clone(),
                });

            return Ok(DeleteWorkspaceResponse {
                workspace_id: workspace_id.to_string(),
                operation: completed_operation,
            });
        }

        self.spawn_delete_runner(&operation);

        Ok(DeleteWorkspaceResponse {
            workspace_id: workspace_id.to_string(),
            operation,
        })
    }

    pub async fn get_running_lifecycle_operations(
        &self,
    ) -> Result<Vec<LifecycleOperation>, ProvisionedRemoteError> {
        self.lifecycle_journal
            .list_running()
            .await
            .map_err(|error| map_lifecycle_journal_error(error, &String::new()))
    }

    pub async fn get_latest_lifecycle_operation(
        &self,
        workspace_id: &str,
    ) -> Result<Option<LifecycleOperation>, ProvisionedRemoteError> {
        let workspace_id = workspace_id.to_string();
        self.lifecycle_journal
            .latest_for_workspace(&workspace_id)
            .await
            .map_err(|error| map_lifecycle_journal_error(error, &workspace_id))
    }

    pub async fn find_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<Workspace>, ProvisionedRemoteError> {
        self.workspace_repository
            .find_workspace_by_id(workspace_id)
            .await
            .map_err(map_workspace_catalog_error)
    }

    pub async fn mark_running_operations_stale(&self) -> Result<(), ProvisionedRemoteError> {
        let operations = self.get_running_lifecycle_operations().await?;

        for operation in operations {
            let payload = payload_with_app_interrupted_error(&operation.payload);

            let workspace = match self.find_workspace(&operation.workspace_id).await? {
                Some(mut workspace) => {
                    let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
                    workspace.state = interrupted_state_for_resources(&runtime.resources);
                    Some(
                        self.workspace_repository
                            .update_workspace(&workspace)
                            .await
                            .map_err(map_workspace_catalog_error)?,
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
                .map_err(|error| map_lifecycle_journal_error(error, &operation.workspace_id))?;

            self.event_sink
                .emit(ProvisionedRemoteEvent::LifecycleOperationChanged {
                    workspace_id: stale_operation.workspace_id.clone(),
                    operation_id: stale_operation.operation_id.clone(),
                    operation: stale_operation,
                });
            if let Some(workspace) = workspace {
                self.event_sink
                    .emit(ProvisionedRemoteEvent::WorkspaceChanged {
                        workspace_id: workspace.id.clone(),
                        workspace,
                    });
            }
        }

        Ok(())
    }

    async fn start_lifecycle_operation(
        &self,
        workspace_id: &str,
        payload: &LifecycleOperationPayload,
    ) -> Result<(Workspace, LifecycleOperation), ProvisionedRemoteError> {
        let workspace = self.load_workspace_required(workspace_id).await?;
        let workspace_id = workspace.id.clone();

        if self
            .lifecycle_journal
            .find_running_by_workspace(&workspace_id)
            .await
            .map_err(|error| map_lifecycle_journal_error(error, &workspace_id))?
            .is_some()
        {
            return Err(ProvisionedRemoteError::LifecycleOperationAlreadyRunning { workspace_id });
        }

        let operation = self
            .lifecycle_journal
            .create_operation(&workspace_id, payload)
            .await
            .map_err(|error| map_lifecycle_journal_error(error, &workspace_id))?;

        self.event_sink
            .emit(ProvisionedRemoteEvent::LifecycleOperationChanged {
                workspace_id: operation.workspace_id.clone(),
                operation_id: operation.operation_id.clone(),
                operation: operation.clone(),
            });

        Ok((workspace, operation))
    }

    async fn load_workspace_required(
        &self,
        workspace_id: &str,
    ) -> Result<Workspace, ProvisionedRemoteError> {
        self.find_workspace(workspace_id)
            .await?
            .ok_or(ProvisionedRemoteError::WorkspaceNotFound)
    }

    fn spawn_provision_runner(&self, operation: &LifecycleOperation) {
        let operation_id = operation.operation_id.clone();
        if self
            .lifecycle_operation_registry
            .try_register(&operation.operation_id)
        {
            #[cfg(test)]
            if !self.spawn_lifecycle_runners {
                self.lifecycle_operation_registry
                    .complete(&operation.operation_id);
                return;
            }

            let registry = self.lifecycle_operation_registry.clone();
            let workspace_repository = self.workspace_repository.clone();
            let lifecycle_journal = self.lifecycle_journal.clone();
            let provider_registry = self.provider_registry.clone();
            let event_sink = self.event_sink.clone();
            let provisioner_poll_interval = self.provisioner_poll_interval;
            spawn_lifecycle_task(async move {
                let result = super::lifecycle::provision::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    &provider_registry,
                    &event_sink,
                    provisioner_poll_interval,
                )
                .await;
                complete_background_runner(&registry, &operation_id, result);
            });
        }
    }

    fn spawn_cleanup_runner(&self, operation: &LifecycleOperation) {
        let operation_id = operation.operation_id.clone();
        if self
            .lifecycle_operation_registry
            .try_register(&operation.operation_id)
        {
            #[cfg(test)]
            if !self.spawn_lifecycle_runners {
                self.lifecycle_operation_registry
                    .complete(&operation.operation_id);
                return;
            }

            let registry = self.lifecycle_operation_registry.clone();
            let workspace_repository = self.workspace_repository.clone();
            let lifecycle_journal = self.lifecycle_journal.clone();
            let provider_registry = self.provider_registry.clone();
            let event_sink = self.event_sink.clone();
            spawn_lifecycle_task(async move {
                let result = super::lifecycle::cleanup::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    &provider_registry,
                    &event_sink,
                )
                .await;
                complete_background_runner(&registry, &operation_id, result);
            });
        }
    }

    fn spawn_delete_runner(&self, operation: &LifecycleOperation) {
        let operation_id = operation.operation_id.clone();
        if self
            .lifecycle_operation_registry
            .try_register(&operation.operation_id)
        {
            #[cfg(test)]
            if !self.spawn_lifecycle_runners {
                self.lifecycle_operation_registry
                    .complete(&operation.operation_id);
                return;
            }

            let registry = self.lifecycle_operation_registry.clone();
            let workspace_repository = self.workspace_repository.clone();
            let lifecycle_journal = self.lifecycle_journal.clone();
            let provider_registry = self.provider_registry.clone();
            let event_sink = self.event_sink.clone();
            spawn_lifecycle_task(async move {
                let result = super::lifecycle::delete::run_once(
                    &operation_id,
                    &workspace_repository,
                    &lifecycle_journal,
                    &provider_registry,
                    &event_sink,
                )
                .await;
                complete_background_runner(&registry, &operation_id, result);
            });
        }
    }

    #[cfg(test)]
    pub async fn run_provision_once_for_test(
        &self,
        operation_id: &str,
    ) -> Result<(), ProvisionedRemoteError> {
        let operation_id = operation_id.to_string();
        let result = super::lifecycle::provision::run_once(
            &operation_id,
            &self.workspace_repository,
            &self.lifecycle_journal,
            &self.provider_registry,
            &self.event_sink,
            Duration::ZERO,
        )
        .await;
        self.lifecycle_operation_registry.complete(&operation_id);
        result
    }

    #[cfg(test)]
    pub async fn run_cleanup_once_for_test(
        &self,
        operation_id: &str,
    ) -> Result<(), ProvisionedRemoteError> {
        let operation_id = operation_id.to_string();
        let result = super::lifecycle::cleanup::run_once(
            &operation_id,
            &self.workspace_repository,
            &self.lifecycle_journal,
            &self.provider_registry,
            &self.event_sink,
        )
        .await;
        self.lifecycle_operation_registry.complete(&operation_id);
        result
    }

    #[cfg(test)]
    pub async fn run_delete_once_for_test(
        &self,
        operation_id: &str,
    ) -> Result<(), ProvisionedRemoteError> {
        let operation_id = operation_id.to_string();
        let result = super::lifecycle::delete::run_once(
            &operation_id,
            &self.workspace_repository,
            &self.lifecycle_journal,
            &self.provider_registry,
            &self.event_sink,
        )
        .await;
        self.lifecycle_operation_registry.complete(&operation_id);
        result
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

pub(super) fn map_workspace_catalog_error(error: WorkspaceCatalogError) -> ProvisionedRemoteError {
    match error {
        WorkspaceCatalogError::WorkspaceAlreadyExists => {
            ProvisionedRemoteError::WorkspaceAlreadyExists
        }
        WorkspaceCatalogError::WorkspaceNotFound => ProvisionedRemoteError::WorkspaceNotFound,
        WorkspaceCatalogError::Corrupt => ProvisionedRemoteError::StorageUnavailable,
        WorkspaceCatalogError::StorageUnavailable
        | WorkspaceCatalogError::MigrationFailed
        | WorkspaceCatalogError::QueryFailed
        | WorkspaceCatalogError::SchemaMismatch => ProvisionedRemoteError::StorageUnavailable,
    }
}

fn complete_background_runner(
    registry: &LifecycleOperationRegistry,
    operation_id: &str,
    result: Result<(), ProvisionedRemoteError>,
) {
    // run_once records provider/domain failures in the lifecycle journal once the
    // operation is loaded. Remaining errors are journal/storage failures where a
    // durable terminal write could not be guaranteed; the persisted Running row
    // continues to block future starts until stale-operation recovery handles it.
    let _unrecoverable = result;
    registry.complete(&operation_id.to_string());
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            lifecycle_operation::{
                LifecycleOperationPayload, LifecycleOperationState,
                ProvisionedRemoteLifecycleOperationPayload,
            },
            provider::GpuCloudProviderId,
            provisioned_remote::{
                ProvisionedRemoteLifecycleError, ProvisionedRemoteProvisionerStatus,
                ProvisionedRemoteRuntime, ProvisionedRemoteVolumeSnapshot,
            },
            workspace::{WorkspaceCleanupRequiredReason, WorkspaceRuntime, WorkspaceState},
        },
        lifecycle_journal::LifecycleJournalRepository,
        provisioned_remote::test_support::{
            block_on, draft_create_request, placement_options, service_with_state,
            service_with_state_and_workspace_repository, service_without_lifecycle_spawning,
            InMemoryWorkspaceRepository, ProviderState, WorkspaceRepositoryState,
        },
        workspace_catalog::WorkspaceCatalogRepository,
    };
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[test]
    fn create_workspace_persists_not_provisioned_workspace_without_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(state.clone());

        let workspace = block_on(service.create_workspace(draft_create_request("workspace-1")))
            .expect("workspace should be created");

        assert_eq!(workspace.id, "workspace-1");
        assert_eq!(workspace.state, WorkspaceState::NotProvisioned);
        let WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
            placement,
            resources,
        }) = &workspace.runtime;
        assert_eq!(placement.gpu_cloud_provider_id, GpuCloudProviderId::Runpod);
        assert!(resources.is_empty());
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
    fn create_workspace_rejects_unsupported_provider_without_persisting_or_provider_calls() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(state.clone());
        let mut request = draft_create_request("workspace-1");
        request
            .workflow_preset
            .remote_runtime_requirements
            .provider_requirements
            .clear();

        let error = block_on(service.create_workspace(request)).expect_err("request should fail");

        assert_eq!(
            error,
            crate::provisioned_remote::errors::ProvisionedRemoteError::InvalidRuntimeState
        );
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
    fn corrupt_workspace_catalog_error_maps_to_storage_unavailable() {
        assert_eq!(
            super::map_workspace_catalog_error(
                crate::workspace_catalog::WorkspaceCatalogError::Corrupt
            ),
            crate::provisioned_remote::errors::ProvisionedRemoteError::StorageUnavailable
        );
    }

    #[test]
    fn get_provider_placement_options_returns_selected_provider_options() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(state.clone());

        let options = block_on(service.get_provider_placement_options(GpuCloudProviderId::Runpod))
            .expect("placement options should resolve");

        assert_eq!(options, placement_options());
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec!["get_provider_placement_options"]
        );
    }

    #[test]
    fn service_uses_nonzero_production_provisioner_poll_interval() {
        let service = service_with_state(Arc::new(Mutex::new(ProviderState::default())));

        assert_eq!(service.provisioner_poll_interval(), Duration::from_secs(5));
    }

    #[tokio::test]
    async fn provision_workspace_creates_running_operation_and_keeps_workspace_state_unchanged() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        let workspace = service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: None,
                    error: None,
                }
            )
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
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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

        assert_eq!(
            error,
            crate::provisioned_remote::errors::ProvisionedRemoteError::LifecycleOperationAlreadyRunning {
                workspace_id: "workspace-1".to_string()
            }
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_steps_updates_resources_and_completes_workspace() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![
                ProvisionedRemoteProvisionerStatus::Running,
                ProvisionedRemoteProvisionerStatus::Succeeded,
            ],
            ..ProviderState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_workspace(draft_create_request("workspace-1"))
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
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        assert_eq!(
            runtime
                .resources
                .volume
                .as_ref()
                .map(|volume| volume.id.as_str()),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner, None);
        assert_eq!(
            runtime
                .resources
                .endpoint
                .as_ref()
                .map(|endpoint| endpoint.id.as_str()),
            Some("endpoint")
        );

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Completed);
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_volume",
                "start_provisioner",
                "get_provisioner_status",
                "get_provisioner_status",
                "terminate_provisioner",
                "create_endpoint",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_failure_preserves_resources_and_sets_cleanup_required() {
        let state = Arc::new(Mutex::new(ProviderState {
            start_provisioner_error: Some(
                crate::provisioned_remote::errors::ProvisionedRemoteError::ProvisionerUnavailable,
            ),
            ..ProviderState::default()
        }));
        let service = service_without_lifecycle_spawning(state);
        service
            .create_workspace(draft_create_request("workspace-1"))
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
        assert_eq!(
            workspace.state,
            WorkspaceState::CleanupRequired {
                reason: WorkspaceCleanupRequiredReason::ProvisionFailed
            }
        );
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        assert_eq!(
            runtime
                .resources
                .volume
                .as_ref()
                .map(|volume| volume.id.as_str()),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner, None);
        assert_eq!(runtime.resources.endpoint, None);

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: Some(crate::domain::provisioned_remote::ProvisionedRemoteProvisionStep::StartProvisioner),
                    error: Some(ProvisionedRemoteLifecycleError::ProvisionerUnavailable),
                }
            )
        );
    }

    #[tokio::test]
    async fn provision_runner_failed_status_terminates_provisioner_without_endpoint() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![ProvisionedRemoteProvisionerStatus::Failed],
            ..ProviderState::default()
        }));
        let service = service_without_lifecycle_spawning(state.clone());
        service
            .create_workspace(draft_create_request("workspace-1"))
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
        assert_eq!(
            workspace.state,
            WorkspaceState::CleanupRequired {
                reason: WorkspaceCleanupRequiredReason::ProvisionFailed
            }
        );
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &workspace.runtime;
        assert_eq!(
            runtime
                .resources
                .volume
                .as_ref()
                .map(|volume| volume.id.as_str()),
            Some("volume")
        );
        assert_eq!(runtime.resources.provisioner, None);
        assert_eq!(runtime.resources.endpoint, None);

        let latest = service
            .get_latest_lifecycle_operation("workspace-1")
            .await
            .expect("operation read should succeed")
            .expect("operation should exist");
        assert_eq!(latest.state, LifecycleOperationState::Failed);
        assert_eq!(
            latest.payload,
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: Some(
                        crate::domain::provisioned_remote::ProvisionedRemoteProvisionStep::TerminateProvisioner
                    ),
                    error: Some(ProvisionedRemoteLifecycleError::ProvisionerFailed),
                }
            )
        );
        assert_eq!(
            state.lock().expect("state lock").calls,
            vec![
                "create_volume",
                "start_provisioner",
                "get_provisioner_status",
                "terminate_provisioner",
            ]
        );
    }

    #[tokio::test]
    async fn provision_workspace_spawned_runner_executes_provider_flow() {
        let state = Arc::new(Mutex::new(ProviderState {
            provisioner_status_results: vec![ProvisionedRemoteProvisionerStatus::Succeeded],
            ..ProviderState::default()
        }));
        let service = service_with_state(state.clone());
        service
            .create_workspace(draft_create_request("workspace-1"))
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
                "create_volume",
                "start_provisioner",
                "get_provisioner_status",
                "terminate_provisioner",
                "create_endpoint",
            ]
        );
    }

    #[tokio::test]
    async fn provision_runner_marks_failed_when_workspace_missing_after_operation_created() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: Some(
                        crate::domain::provisioned_remote::ProvisionedRemoteProvisionStep::CreateVolume
                    ),
                    error: Some(ProvisionedRemoteLifecycleError::InvalidRuntimeState),
                }
            )
        );
    }

    #[tokio::test]
    async fn cleanup_workspace_creates_cleanup_operation_without_changing_workspace_state() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        let workspace = service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");

        let response = service
            .cleanup_workspace("workspace-1")
            .await
            .expect("cleanup should start");

        assert_eq!(response.workspace, workspace);
        assert_eq!(
            response.operation.payload,
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                    step: None,
                    error: None,
                }
            )
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
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Cleanup {
                    step: Some(
                        crate::domain::provisioned_remote::ProvisionedRemoteCleanupStep::DeleteEndpoint
                    ),
                    error: Some(ProvisionedRemoteLifecycleError::InvalidRuntimeState),
                }
            )
        );
    }

    #[tokio::test]
    async fn delete_workspace_without_resources_completes_and_deletes_immediately() {
        let service = service_with_state(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Delete {
                    step: Some(
                        crate::domain::provisioned_remote::ProvisionedRemoteDeleteStep::DeleteLocalWorkspace
                    ),
                    error: None,
                }
            )
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
            Arc::new(Mutex::new(ProviderState::default())),
            InMemoryWorkspaceRepository::with_state(workspace_state.clone()),
        );
        service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        workspace_state
            .lock()
            .expect("workspace state lock should succeed")
            .delete_workspace_error =
            Some(crate::workspace_catalog::WorkspaceCatalogError::QueryFailed);

        let error = service
            .delete_workspace("workspace-1")
            .await
            .expect_err("delete should fail when workspace delete fails");

        assert_eq!(
            error,
            crate::provisioned_remote::errors::ProvisionedRemoteError::StorageUnavailable
        );
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
        assert_eq!(latest.state, LifecycleOperationState::Completed);
    }

    #[tokio::test]
    async fn delete_runner_completes_when_workspace_missing_after_operation_created() {
        let service = service_with_state(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: None,
                error: None,
            },
        );
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
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let service = service_with_state(state.clone());
        let mut workspace = service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.volume = Some(ProvisionedRemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace should update");
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: None,
                error: None,
            },
        );
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
            vec!["delete_volume"]
        );
    }

    #[tokio::test]
    async fn delete_runner_preserves_lifecycle_row_when_workspace_delete_fails() {
        let state = Arc::new(Mutex::new(ProviderState::default()));
        let workspace_state = Arc::new(Mutex::new(WorkspaceRepositoryState::default()));
        let service = service_with_state_and_workspace_repository(
            state,
            InMemoryWorkspaceRepository::with_state(workspace_state.clone()),
        );
        let mut workspace = service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.volume = Some(ProvisionedRemoteVolumeSnapshot {
            id: "volume".to_string(),
        });
        service
            .workspace_repository
            .update_workspace(&workspace)
            .await
            .expect("workspace should update");
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: None,
                error: None,
            },
        );
        let operation_id = service
            .lifecycle_journal
            .create_operation(&"workspace-1".to_string(), &payload)
            .await
            .expect("delete operation should be created")
            .operation_id;
        workspace_state
            .lock()
            .expect("workspace state lock should succeed")
            .delete_workspace_error =
            Some(crate::workspace_catalog::WorkspaceCatalogError::QueryFailed);

        let error = service
            .run_delete_once_for_test(&operation_id)
            .await
            .expect_err("delete runner should fail when workspace delete fails");

        assert_eq!(
            error,
            crate::provisioned_remote::errors::ProvisionedRemoteError::StorageUnavailable
        );
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
        assert_eq!(latest.state, LifecycleOperationState::Completed);
    }

    #[tokio::test]
    async fn get_running_lifecycle_operations_returns_started_operations() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: None,
                    error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
                }
            )
        );
        assert_eq!(stale.operation_id, operation.operation_id);
        let workspace = service
            .find_workspace("workspace-1")
            .await
            .expect("workspace should load")
            .expect("workspace should exist");
        assert_eq!(
            workspace.state,
            WorkspaceState::Invalid {
                reason:
                    crate::domain::workspace::WorkspaceRuntimeInvalidReason::OperationInterrupted,
            }
        );
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_workspace_cleanup_required_when_resources_exist() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        let mut workspace = service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let WorkspaceRuntime::ProvisionedRemote(runtime) = &mut workspace.runtime;
        runtime.resources.volume = Some(ProvisionedRemoteVolumeSnapshot {
            id: "volume-1".to_string(),
        });
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
        assert_eq!(
            workspace.state,
            WorkspaceState::CleanupRequired {
                reason: WorkspaceCleanupRequiredReason::OperationInterrupted,
            }
        );
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_delete_stale_when_workspace_is_missing() {
        let service = service_with_state(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
            .await
            .expect("workspace should be created");
        let payload = LifecycleOperationPayload::ProvisionedRemote(
            ProvisionedRemoteLifecycleOperationPayload::Delete {
                step: None,
                error: None,
            },
        );
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Delete {
                    step: None,
                    error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
                }
            )
        );
    }

    #[tokio::test]
    async fn mark_running_operations_stale_marks_provision_stale_when_workspace_is_missing() {
        let service =
            service_without_lifecycle_spawning(Arc::new(Mutex::new(ProviderState::default())));
        service
            .create_workspace(draft_create_request("workspace-1"))
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
            LifecycleOperationPayload::ProvisionedRemote(
                ProvisionedRemoteLifecycleOperationPayload::Provision {
                    step: None,
                    error: Some(ProvisionedRemoteLifecycleError::AppInterrupted),
                }
            )
        );
    }
}
