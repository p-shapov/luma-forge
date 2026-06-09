use std::sync::Arc;

use crate::{
    domain::{
        placement::{RemotePlacementOptions, RemotePlacementPlan},
        provider::GpuCloudProviderId,
        provisioned_remote::{ProvisionedRemoteResources, ProvisionedRemoteRuntime},
        workflow_preset::WorkflowPreset,
        workspace::{Workspace, WorkspaceRuntime, WorkspaceState},
    },
    workspace_catalog::{WorkspaceCatalogError, WorkspaceCatalogRepository},
};

use super::{
    errors::ProvisionedRemoteError,
    events::{NoopProvisionedRemoteEventSink, ProvisionedRemoteEvent, ProvisionedRemoteEventSink},
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
    event_sink: Arc<dyn ProvisionedRemoteEventSink>,
}

impl<W, L> ProvisionedRemoteService<W, L>
where
    W: WorkspaceCatalogRepository,
    L: crate::lifecycle_journal::LifecycleJournalRepository,
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
            event_sink: Arc::new(NoopProvisionedRemoteEventSink),
        }
    }

    pub fn with_event_sink(mut self, event_sink: Arc<dyn ProvisionedRemoteEventSink>) -> Self {
        self.event_sink = event_sink;
        self
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
}

fn map_workspace_catalog_error(error: WorkspaceCatalogError) -> ProvisionedRemoteError {
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

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            provider::GpuCloudProviderId,
            provisioned_remote::ProvisionedRemoteRuntime,
            workspace::{WorkspaceRuntime, WorkspaceState},
        },
        provisioned_remote::test_support::{
            block_on, draft_create_request, placement_options, service_with_state, ProviderState,
        },
        workspace_catalog::WorkspaceCatalogRepository,
    };
    use std::sync::{Arc, Mutex};

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
}
