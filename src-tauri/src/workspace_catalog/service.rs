use crate::domain::workspace::{Workspace, WorkspaceCatalog};

use super::{errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository};

pub struct WorkspaceCatalogService<R> {
    repository: R,
}

impl<R> WorkspaceCatalogService<R>
where
    R: WorkspaceCatalogRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_workspaces(&self) -> Result<WorkspaceCatalog, WorkspaceCatalogError> {
        self.repository.list_workspaces().await
    }

    pub async fn find_workspace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<Workspace>, WorkspaceCatalogError> {
        self.repository.find_workspace_by_id(id).await
    }

    pub async fn insert_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        self.repository.insert_workspace(workspace).await
    }

    pub async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        self.repository.update_workspace(workspace).await
    }

    pub async fn delete_workspace(&self, id: &str) -> Result<(), WorkspaceCatalogError> {
        self.repository.delete_workspace(id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        domain::{
            placement::{
                Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
                RemotePlacementPlan,
            },
            provider::GpuCloudProviderId,
            runtime_contract::RuntimeContractReference,
            workflow_preset::{
                ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
                RemoteRuntimeRequirements, WorkflowExecutionType, WorkflowPreset,
            },
            workspace::{
                RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
                RemoteWorkspaceResources, Workspace, WorkspaceCatalog, WorkspaceRuntime,
            },
        },
        shared::AppFuture,
    };

    use super::WorkspaceCatalogService;
    use crate::workspace_catalog::{
        errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository,
    };

    struct FakeRepository {
        calls: Arc<Mutex<Vec<&'static str>>>,
        result: Result<(), WorkspaceCatalogError>,
    }

    impl FakeRepository {
        fn new(
            calls: Arc<Mutex<Vec<&'static str>>>,
            result: Result<(), WorkspaceCatalogError>,
        ) -> Self {
            Self { calls, result }
        }

        fn record(&self, method: &'static str) {
            self.calls.lock().unwrap().push(method);
        }

        fn cloned_error<T>(&self) -> Option<Result<T, WorkspaceCatalogError>> {
            self.result.as_ref().err().cloned().map(Err)
        }
    }

    impl WorkspaceCatalogRepository for FakeRepository {
        fn list_workspaces<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("list_workspaces");
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(WorkspaceCatalog {
                    workspaces: vec![workspace("workspace-1")],
                })
            })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            id: &'a str,
        ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("find_workspace_by_id");
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(Some(workspace(id)))
            })
        }

        fn insert_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("insert_workspace");
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(workspace.clone())
            })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("update_workspace");
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(workspace.clone())
            })
        }

        fn delete_workspace<'a>(
            &'a self,
            _id: &'a str,
        ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("delete_workspace");
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn list_workspaces_delegates_to_repository() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(calls.clone(), Ok(())));

        let result = service.list_workspaces().await.unwrap();

        assert_eq!(
            result,
            WorkspaceCatalog {
                workspaces: vec![workspace("workspace-1")],
            }
        );
        assert_eq!(*calls.lock().unwrap(), vec!["list_workspaces"]);
    }

    #[tokio::test]
    async fn service_preserves_repository_errors() {
        let error = WorkspaceCatalogError::QueryFailed;
        let workspace = workspace("workspace-1");

        let calls = Arc::new(Mutex::new(Vec::new()));
        let service =
            WorkspaceCatalogService::new(FakeRepository::new(calls.clone(), Err(error.clone())));

        assert_eq!(service.list_workspaces().await, Err(error.clone()));
        assert_eq!(
            service.find_workspace_by_id("workspace-1").await,
            Err(error.clone())
        );
        assert_eq!(
            service.insert_workspace(&workspace).await,
            Err(error.clone())
        );
        assert_eq!(
            service.update_workspace(&workspace).await,
            Err(error.clone())
        );
        assert_eq!(service.delete_workspace("workspace-1").await, Err(error));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "list_workspaces",
                "find_workspace_by_id",
                "insert_workspace",
                "update_workspace",
                "delete_workspace",
            ]
        );
    }

    fn workspace(id: &str) -> Workspace {
        Workspace {
            id: id.to_string(),
            workflow_preset: WorkflowPreset {
                id: "workflow-1".to_string(),
                version: "1".to_string(),
                name: "Workflow 1".to_string(),
                execution_type: WorkflowExecutionType::T2i,
                requires_hugging_face_api_key: false,
                remote_runtime_requirements: RemoteRuntimeRequirements {
                    required_base_volume_size_bytes: 1,
                    provider_requirements: vec![RemoteProviderRuntimeRequirements {
                        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                        endpoint_contract: RuntimeContractReference {
                            id: "endpoint-contract".to_string(),
                            version: "1".to_string(),
                        },
                        provisioner_contract: RuntimeContractReference {
                            id: "provisioner-contract".to_string(),
                            version: "1".to_string(),
                        },
                    }],
                },
                required_model_assets: vec![ModelAsset {
                    id: "asset-1".to_string(),
                    name: "Asset 1".to_string(),
                    download_source: ModelAssetSource::Huggingface {
                        repository_id: "owner/repository".to_string(),
                        file_path: "model.safetensors".to_string(),
                        revision: "main".to_string(),
                    },
                    install_comfyui_relative_path: "models/model.safetensors".to_string(),
                }],
            },
            runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
                remote_placement: RemotePlacementPlan {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    datacenter_id: "datacenter-1".to_string(),
                    gpu_id: "gpu-1".to_string(),
                    remote_volume_size_bytes: 1,
                    remote_capabilities: RemotePlacementCapabilities {
                        remote_endpoint_keep_alive: Capability::Supported(
                            RemoteEndpointKeepAliveLimits {
                                default_seconds: 60,
                                min_seconds: 0,
                                max_seconds: 3600,
                            },
                        ),
                    },
                },
                remote_provisioning: RemoteProvisioningState {
                    status: RemoteProvisioningStatus::NotStarted,
                    percent: None,
                },
                remote_resources: RemoteWorkspaceResources {
                    remote_volume: None,
                    remote_provisioner: None,
                    remote_endpoint: None,
                },
            }),
        }
    }
}
