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
            provisioned_remote::GpuCloudProviderId,
            provisioned_remote::{ProvisionedRemoteResources, ProvisionedRemoteRuntime},
            provisioned_remote::{RemoteEndpointKeepAliveLimits, RemotePlacementPlan},
            runtime_contract::RuntimeContractReference,
            workflow_preset::{
                ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
                RemoteRuntimeRequirements, WorkflowExecutionType, WorkflowPreset,
            },
            workspace::{Workspace, WorkspaceCatalog, WorkspaceRuntime, WorkspaceState},
        },
        shared::AppFuture,
    };

    use super::WorkspaceCatalogService;
    use crate::workspace_catalog::{
        errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository,
    };

    struct FakeRepository {
        calls: Arc<Mutex<Vec<&'static str>>>,
        ids: Arc<Mutex<Vec<String>>>,
        workspaces: Arc<Mutex<Vec<Workspace>>>,
        result: Result<(), WorkspaceCatalogError>,
    }

    impl FakeRepository {
        fn new(
            calls: Arc<Mutex<Vec<&'static str>>>,
            ids: Arc<Mutex<Vec<String>>>,
            workspaces: Arc<Mutex<Vec<Workspace>>>,
            result: Result<(), WorkspaceCatalogError>,
        ) -> Self {
            Self {
                calls,
                ids,
                workspaces,
                result,
            }
        }

        fn record(&self, method: &'static str) {
            self.calls.lock().unwrap().push(method);
        }

        fn record_id(&self, id: &str) {
            self.ids.lock().unwrap().push(id.to_string());
        }

        fn record_workspace(&self, workspace: &Workspace) {
            self.workspaces.lock().unwrap().push(workspace.clone());
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
                self.record_id(id);
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
                self.record_workspace(workspace);
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
                self.record_workspace(workspace);
                if let Some(error) = self.cloned_error() {
                    return error;
                }

                Ok(workspace.clone())
            })
        }

        fn delete_workspace<'a>(
            &'a self,
            id: &'a str,
        ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
            Box::pin(async move {
                self.record("delete_workspace");
                self.record_id(id);
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
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids,
            workspaces,
            Ok(()),
        ));

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
    async fn find_workspace_by_id_delegates_id_and_result() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids.clone(),
            workspaces,
            Ok(()),
        ));

        let result = service.find_workspace_by_id("workspace-2").await.unwrap();

        assert_eq!(result, Some(workspace("workspace-2")));
        assert_eq!(*calls.lock().unwrap(), vec!["find_workspace_by_id"]);
        assert_eq!(*ids.lock().unwrap(), vec!["workspace-2"]);
    }

    #[tokio::test]
    async fn insert_workspace_delegates_workspace_and_result() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids,
            workspaces.clone(),
            Ok(()),
        ));
        let workspace = workspace("workspace-3");

        let result = service.insert_workspace(&workspace).await.unwrap();

        assert_eq!(result, workspace);
        assert_eq!(*calls.lock().unwrap(), vec!["insert_workspace"]);
        assert_eq!(*workspaces.lock().unwrap(), vec![workspace]);
    }

    #[tokio::test]
    async fn update_workspace_delegates_workspace_and_result() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids,
            workspaces.clone(),
            Ok(()),
        ));
        let workspace = workspace("workspace-4");

        let result = service.update_workspace(&workspace).await.unwrap();

        assert_eq!(result, workspace);
        assert_eq!(*calls.lock().unwrap(), vec!["update_workspace"]);
        assert_eq!(*workspaces.lock().unwrap(), vec![workspace]);
    }

    #[tokio::test]
    async fn delete_workspace_delegates_id() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids.clone(),
            workspaces,
            Ok(()),
        ));

        let result = service.delete_workspace("workspace-5").await;

        assert_eq!(result, Ok(()));
        assert_eq!(*calls.lock().unwrap(), vec!["delete_workspace"]);
        assert_eq!(*ids.lock().unwrap(), vec!["workspace-5"]);
    }

    #[tokio::test]
    async fn service_preserves_repository_errors() {
        let error = WorkspaceCatalogError::QueryFailed;
        let workspace = workspace("workspace-1");

        let calls = Arc::new(Mutex::new(Vec::new()));
        let ids = Arc::new(Mutex::new(Vec::new()));
        let workspaces = Arc::new(Mutex::new(Vec::new()));
        let service = WorkspaceCatalogService::new(FakeRepository::new(
            calls.clone(),
            ids,
            workspaces,
            Err(error.clone()),
        ));

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
                id: "preset".to_string(),
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
            state: WorkspaceState::NotProvisioned,
            runtime: WorkspaceRuntime::ProvisionedRemote(ProvisionedRemoteRuntime {
                placement: RemotePlacementPlan {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    datacenter_id: "datacenter-1".to_string(),
                    gpu_id: "gpu-1".to_string(),
                    volume_size_bytes: 1,
                    keep_alive_limits: Some(RemoteEndpointKeepAliveLimits {
                        default_seconds: 60,
                        min_seconds: 0,
                        max_seconds: 3600,
                    }),
                },
                resources: ProvisionedRemoteResources {
                    volume_id: None,
                    provisioner_id: None,
                    endpoint_id: None,
                },
            }),
        }
    }
}
