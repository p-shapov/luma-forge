use std::{future::Future, sync::Arc, time::Duration};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    events::ApplicationEventSink,
    runtimes::{
        ports::{RuntimeOperationRepository, RuntimeTransitionRepository},
        RuntimeError, RuntimeOperation, RuntimeOperationState, RuntimeState,
        RuntimeTransitionContext,
    },
    secrets::{SecretKind, SecretStore},
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository},
        Workspace,
    },
};

use super::{RunpodPlacement, RunpodRuntime, RunpodRuntimeCatalog, RunpodRuntimeProvider};

#[derive(luma_diagnostics::DiagnosticDebug, Clone, Copy)]
enum LifecycleTermination {
    BodyError,
    Deadline,
    Panic,
    Cancelled,
}

pub(super) fn mark_failed(workspace: &mut Workspace) -> Result<(), RuntimeError> {
    runpod(workspace)?;
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    match runtime.state {
        RuntimeState::Provisioning | RuntimeState::CleaningUp => {
            runtime.state = RuntimeState::Failed;
            Ok(())
        }
        RuntimeState::Ready | RuntimeState::Failed => Err(RuntimeError::InvalidTransition),
    }
}

pub(super) fn runpod(workspace: &Workspace) -> Result<&RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_ref()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod()
        .ok_or(RuntimeError::InvalidTransition)
}

pub(super) fn runpod_mut(workspace: &mut Workspace) -> Result<&mut RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod_mut()
        .ok_or(RuntimeError::InvalidTransition)
}

#[derive(Clone)]
pub struct RunpodRuntimeService {
    pub(super) workspaces: Arc<dyn WorkspaceRepository>,
    pub(super) operations: Arc<dyn RuntimeOperationRepository>,
    pub(super) workflows: Arc<dyn WorkflowCatalog>,
    pub(super) runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    pub(super) secrets: Arc<dyn SecretStore>,
    pub(super) provider: Arc<dyn RunpodRuntimeProvider>,
    pub(super) transitions: RuntimeTransitionContext,
}

pub struct RunpodRuntimeServiceDependencies {
    pub workspaces: Arc<dyn WorkspaceRepository>,
    pub operations: Arc<dyn RuntimeOperationRepository>,
    pub workflows: Arc<dyn WorkflowCatalog>,
    pub transitions: Arc<dyn RuntimeTransitionRepository>,
    pub runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    pub secrets: Arc<dyn SecretStore>,
    pub provider: Arc<dyn RunpodRuntimeProvider>,
    pub events: Arc<dyn ApplicationEventSink>,
}

impl RunpodRuntimeService {
    pub fn new(dependencies: RunpodRuntimeServiceDependencies) -> Self {
        let transitions =
            RuntimeTransitionContext::new(dependencies.transitions, dependencies.events);
        Self {
            workspaces: dependencies.workspaces,
            operations: dependencies.operations,
            workflows: dependencies.workflows,
            runtime_catalog: dependencies.runtime_catalog,
            secrets: dependencies.secrets,
            provider: dependencies.provider,
            transitions,
        }
    }

    #[luma_diagnostics::diagnostic(show_output, show_error)]
    pub async fn placement(&self) -> Result<RunpodPlacement, RuntimeError> {
        let key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RuntimeError::CredentialMissing)?;
        self.provider.placement(&key).await.map_err(Into::into)
    }

    pub(super) fn spawn_supervised<F>(&self, operation_id: Uuid, deadline: Duration, body: F)
    where
        F: Future<Output = Result<(), RuntimeError>> + Send + 'static,
    {
        let body = tokio::spawn(body);
        let service = self.clone();
        tokio::spawn(async move {
            let _ = service.supervise(operation_id, deadline, body).await;
        });
    }

    #[luma_diagnostics::diagnostic(detached, show_error)]
    async fn supervise(
        self,
        #[diagnostic(show)] operation_id: Uuid,
        deadline: Duration,
        mut body: tokio::task::JoinHandle<Result<(), RuntimeError>>,
    ) -> Result<(), RuntimeError> {
        let termination = match tokio::time::timeout(deadline, &mut body).await {
            Ok(Ok(Ok(()))) => return Ok(()),
            Ok(Ok(Err(_))) => LifecycleTermination::BodyError,
            Ok(Err(join_error)) if join_error.is_panic() => LifecycleTermination::Panic,
            Ok(Err(_)) => LifecycleTermination::Cancelled,
            Err(_) => {
                body.abort();
                let _ = body.await;
                LifecycleTermination::Deadline
            }
        };
        let operation = self
            .operations
            .get(operation_id)
            .await
            .map_err(|_| RuntimeError::PersistenceUnavailable)?
            .ok_or(RuntimeError::PersistenceUnavailable)?;
        if operation.state != RuntimeOperationState::Running {
            return Ok(());
        }
        let workspace = self
            .workspaces
            .get(&operation.workspace_id)
            .await
            .map_err(|_| RuntimeError::PersistenceUnavailable)?
            .ok_or(RuntimeError::NotProvisioned)?;
        self.terminalize_supervised_operation(workspace, operation, termination)
            .await
    }

    #[luma_diagnostics::diagnostic(restore = operation.trace_id, show_error)]
    async fn terminalize_supervised_operation(
        &self,
        mut workspace: Workspace,
        mut operation: RuntimeOperation,
        #[diagnostic(show)] _termination: LifecycleTermination,
    ) -> Result<(), RuntimeError> {
        self.fail_transition(&mut workspace, &mut operation).await
    }

    async fn fail_transition(
        &self,
        workspace: &mut Workspace,
        operation: &mut RuntimeOperation,
    ) -> Result<(), RuntimeError> {
        mark_failed(workspace)?;
        operation.fail(OffsetDateTime::now_utc())?;
        self.transitions
            .save(workspace, operation)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::runpod::{
        test_support::ProvisionFakes, RunpodPlacement, RunpodPlacementDatacenter,
        RunpodPlacementGpu,
    };
    use crate::application::runtimes::RuntimeError;

    #[tokio::test]
    async fn placement_reads_the_stored_key_and_returns_normalized_options() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.set_placement(RunpodPlacement {
            max_volume_size_gb: 4_000,
            datacenters: vec![RunpodPlacementDatacenter {
                id: "EU-RO-1".into(),
                name: "EU Romania".into(),
                gpus: vec![RunpodPlacementGpu {
                    id: "NVIDIA RTX 4090".into(),
                    name: "RTX 4090".into(),
                    vram_gb: 24,
                }],
            }],
        });

        let placement = fakes.service().placement().await.unwrap();
        assert_eq!(placement.max_volume_size_gb, 4_000);
        assert_eq!(placement.datacenters[0].gpus[0].vram_gb, 24);
        assert_eq!(fakes.provider.calls(), vec!["placement"]);
    }

    #[tokio::test]
    async fn placement_requires_the_runpod_key_before_calling_provider() {
        let fakes = ProvisionFakes::ready_without_runpod_credential();
        assert_eq!(
            fakes.service().placement().await,
            Err(RuntimeError::CredentialMissing)
        );
        assert!(fakes.provider.calls().is_empty());
    }
}
