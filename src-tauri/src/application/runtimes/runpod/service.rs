use std::sync::Arc;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    events::ApplicationEventSink,
    runtimes::{
        ports::RuntimeTransitionRepository, Runtime, RuntimeContractRequirements, RuntimeKind,
        RuntimeOperation, RuntimeOperationKind, RuntimeProgress, RuntimeProvider, RuntimeState,
        RuntimeTransitionContext, WorkflowDefinition,
    },
    secrets::{SecretKind, SecretStore},
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository},
        Workspace,
    },
};

use super::{
    CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodCleanupStep, RunpodProgress,
    RunpodProvisionStep, RunpodRuntime, RunpodRuntimeCatalog, RunpodRuntimeConfig,
    RunpodRuntimeDefinition, RunpodRuntimeError, RunpodRuntimeProvider, StartProvisionerPod,
};

#[derive(crate::diagnostics::DiagnosticDebug)]
pub struct ProvisionRunpodRuntime {
    #[diagnostic(show)]
    pub workspace_id: String,
    #[diagnostic(show)]
    pub datacenter_id: String,
    #[diagnostic(show)]
    pub gpu_id: String,
    #[diagnostic(show)]
    pub volume_size_gb: u64,
}

fn provision_conflict(state: RuntimeState) -> RunpodRuntimeError {
    match state {
        RuntimeState::Ready => RunpodRuntimeError::AlreadyProvisioned,
        RuntimeState::Failed => RunpodRuntimeError::RuntimeFailed,
        RuntimeState::Provisioning | RuntimeState::CleaningUp => {
            RunpodRuntimeError::OperationInProgress
        }
    }
}

fn begin_cleanup(workspace: &mut Workspace) -> Result<(), RunpodRuntimeError> {
    runpod(workspace)?;
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RunpodRuntimeError::NotProvisioned)?;
    match runtime.state {
        RuntimeState::Ready | RuntimeState::Failed => {
            runtime.state = RuntimeState::CleaningUp;
            Ok(())
        }
        RuntimeState::Provisioning | RuntimeState::CleaningUp => {
            Err(RunpodRuntimeError::OperationInProgress)
        }
    }
}

fn mark_ready(workspace: &mut Workspace) -> Result<(), RunpodRuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RunpodRuntimeError::NotProvisioned)?;
    if runtime.state != RuntimeState::Provisioning {
        return Err(RunpodRuntimeError::InvalidTransition);
    }
    runtime.state = RuntimeState::Ready;
    Ok(())
}

fn mark_failed(workspace: &mut Workspace) -> Result<(), RunpodRuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RunpodRuntimeError::NotProvisioned)?;
    match runtime.state {
        RuntimeState::Provisioning | RuntimeState::CleaningUp => {
            runtime.state = RuntimeState::Failed;
            Ok(())
        }
        RuntimeState::Ready | RuntimeState::Failed => Err(RunpodRuntimeError::InvalidTransition),
    }
}

fn runpod(workspace: &Workspace) -> Result<&RunpodRuntime, RunpodRuntimeError> {
    match workspace.runtime.as_ref().map(|runtime| &runtime.provider) {
        Some(RuntimeProvider::Runpod(runtime)) => Ok(runtime),
        None => Err(RunpodRuntimeError::NotProvisioned),
    }
}

fn runpod_mut(workspace: &mut Workspace) -> Result<&mut RunpodRuntime, RunpodRuntimeError> {
    match workspace
        .runtime
        .as_mut()
        .map(|runtime| &mut runtime.provider)
    {
        Some(RuntimeProvider::Runpod(runtime)) => Ok(runtime),
        None => Err(RunpodRuntimeError::NotProvisioned),
    }
}

#[derive(Clone)]
pub struct RunpodRuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn RunpodRuntimeProvider>,
    transitions: RuntimeTransitionContext,
}

pub struct RunpodRuntimeServiceDependencies {
    pub workspaces: Arc<dyn WorkspaceRepository>,
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
            workflows: dependencies.workflows,
            runtime_catalog: dependencies.runtime_catalog,
            secrets: dependencies.secrets,
            provider: dependencies.provider,
            transitions,
        }
    }

    #[crate::diagnostics::diagnostic]
    pub async fn start_cleanup(
        &self,
        #[diagnostic(show)] mut workspace: Workspace,
    ) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError> {
        let workspace_id = workspace.id.clone();
        let runpod_key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RunpodRuntimeError::CredentialMissing)?;
        begin_cleanup(&mut workspace)?;

        let operation = RuntimeOperation::running(
            Uuid::new_v4(),
            &workspace_id,
            RuntimeKind::Runpod,
            RuntimeOperationKind::Cleanup,
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint)),
            OffsetDateTime::now_utc(),
        );
        self.transitions.save(&workspace, &operation).await?;

        let initial_workspace = workspace.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        tokio::spawn(service.run_cleanup(workspace_id, runpod_key, workspace, operation));

        Ok((initial_workspace, initial_operation))
    }

    #[crate::diagnostics::diagnostic(detached, show_error)]
    async fn run_cleanup(
        self,
        #[diagnostic(show)] _workspace_id: String,
        runpod_key: secrecy::SecretString,
        mut workspace: Workspace,
        mut operation: RuntimeOperation,
    ) -> Result<(), RunpodRuntimeError> {
        if let Some(id) = runpod(&workspace)?.resources.endpoint_id.clone() {
            if let Err(error) = self.provider.delete_endpoint(&runpod_key, &id).await {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
            runpod_mut(&mut workspace)?.resources.endpoint_id = None;
        }
        self.set_cleanup_step(
            &workspace,
            &mut operation,
            RunpodCleanupStep::DeleteTemplate,
        )
        .await?;

        if let Some(id) = runpod(&workspace)?.resources.template_id.clone() {
            if let Err(error) = self.provider.delete_template(&runpod_key, &id).await {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
            runpod_mut(&mut workspace)?.resources.template_id = None;
        }
        self.set_cleanup_step(
            &workspace,
            &mut operation,
            RunpodCleanupStep::TerminateProvisionerPod,
        )
        .await?;

        if let Some(id) = runpod(&workspace)?.resources.provisioner_pod_id.clone() {
            if let Err(error) = self
                .provider
                .terminate_provisioner_pod(&runpod_key, &id)
                .await
            {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
            runpod_mut(&mut workspace)?.resources.provisioner_pod_id = None;
        }
        self.set_cleanup_step(
            &workspace,
            &mut operation,
            RunpodCleanupStep::DeleteNetworkVolume,
        )
        .await?;

        if let Some(id) = runpod(&workspace)?.resources.network_volume_id.clone() {
            if let Err(error) = self.provider.delete_network_volume(&runpod_key, &id).await {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
            runpod_mut(&mut workspace)?.resources.network_volume_id = None;
        }

        operation.succeed(OffsetDateTime::now_utc())?;
        workspace.runtime = None;
        self.transitions.save(&workspace, &operation).await?;
        Ok(())
    }

    #[crate::diagnostics::diagnostic]
    pub async fn recover_interrupted(
        &self,
        operations: Vec<RuntimeOperation>,
    ) -> Result<(), RunpodRuntimeError> {
        for operation in operations {
            self.recover_one(operation).await?;
        }
        Ok(())
    }

    #[crate::diagnostics::diagnostic(restore = operation.trace_id)]
    async fn recover_one(&self, operation: RuntimeOperation) -> Result<(), RunpodRuntimeError> {
        let mut operation = operation;
        if operation.runtime_kind != RuntimeKind::Runpod {
            return Err(RunpodRuntimeError::PersistenceUnavailable);
        }
        let mut workspace = self
            .workspaces
            .get(&operation.workspace_id)
            .await
            .map_err(|_| RunpodRuntimeError::PersistenceUnavailable)?
            .ok_or(RunpodRuntimeError::NotProvisioned)?;
        mark_failed(&mut workspace)?;
        operation.fail(OffsetDateTime::now_utc())?;
        self.transitions.save(&workspace, &operation).await?;
        Ok(())
    }

    #[crate::diagnostics::diagnostic]
    pub async fn start_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRunpodRuntime,
    ) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError> {
        if command.volume_size_gb > 4_000 {
            return Err(RunpodRuntimeError::InvalidTransition);
        }
        let mut workspace = self
            .workspaces
            .get(&command.workspace_id)
            .await
            .map_err(|_| RunpodRuntimeError::PersistenceUnavailable)?
            .ok_or(RunpodRuntimeError::WorkspaceNotFound)?;

        if let Some(runtime) = &workspace.runtime {
            return Err(provision_conflict(runtime.state));
        }

        let workflow = self
            .workflows
            .get(&workspace.workflow.id, &workspace.workflow.revision)
            .await
            .map_err(|_| RunpodRuntimeError::CatalogUnavailable)?
            .ok_or(RunpodRuntimeError::WorkflowNotFound)?;
        let requirements = workflow
            .contract_requirements
            .first()
            .map(|requirements| match requirements {
                RuntimeContractRequirements::Runpod(value) => value,
            })
            .ok_or(RunpodRuntimeError::CatalogUnavailable)?;
        let definition = self
            .runtime_catalog
            .resolve(&workflow.runtime_preset_ref, requirements)
            .await?;
        let runpod_key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RunpodRuntimeError::CredentialMissing)?;
        let hugging_face_api_key = if workflow.summary.requires_hugging_face_api_key {
            Some(
                self.secrets
                    .get(SecretKind::HuggingFaceApiKey)
                    .await?
                    .ok_or(RunpodRuntimeError::CredentialMissing)?,
            )
        } else {
            None
        };

        workspace.runtime = Some(Runtime {
            state: RuntimeState::Provisioning,
            provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
                RunpodRuntimeConfig {
                    datacenter_id: command.datacenter_id.clone(),
                    gpu_id: command.gpu_id.clone(),
                    volume_size_gb: command.volume_size_gb,
                },
            )),
        });
        let operation = RuntimeOperation::running(
            Uuid::new_v4(),
            &command.workspace_id,
            RuntimeKind::Runpod,
            RuntimeOperationKind::Provision,
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
            OffsetDateTime::now_utc(),
        );
        self.transitions.save(&workspace, &operation).await?;

        let initial_workspace = workspace.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        tokio::spawn(service.run_provision(
            command,
            definition,
            workflow,
            runpod_key,
            hugging_face_api_key,
            workspace,
            operation,
        ));

        Ok((initial_workspace, initial_operation))
    }

    #[allow(clippy::too_many_arguments)]
    #[crate::diagnostics::diagnostic(detached, show_error)]
    async fn run_provision(
        self,
        #[diagnostic(show)] command: ProvisionRunpodRuntime,
        definition: RunpodRuntimeDefinition,
        workflow: WorkflowDefinition,
        runpod_key: secrecy::SecretString,
        hugging_face_api_key: Option<secrecy::SecretString>,
        mut workspace: Workspace,
        mut operation: RuntimeOperation,
    ) -> Result<(), RunpodRuntimeError> {
        let volume_id = match self
            .provider
            .create_network_volume(
                &runpod_key,
                CreateNetworkVolume {
                    workspace_id: command.workspace_id.clone(),
                    datacenter_id: command.datacenter_id.clone(),
                    size_gb: command.volume_size_gb,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.network_volume_id = Some(volume_id.clone());
        self.set_provision_step(
            &workspace,
            &mut operation,
            RunpodProvisionStep::StartProvisionerPod,
        )
        .await?;

        let pod_id = match self
            .provider
            .start_provisioner_pod(
                &runpod_key,
                StartProvisionerPod {
                    workspace_id: command.workspace_id.clone(),
                    datacenter_id: command.datacenter_id.clone(),
                    network_volume_id: volume_id.clone(),
                    provisioner_image_ref: definition.provisioner_image_ref,
                    required_model_assets: workflow.model_assets,
                    hugging_face_api_key,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.provisioner_pod_id = Some(pod_id.clone());
        self.set_provision_step(
            &workspace,
            &mut operation,
            RunpodProvisionStep::PollProvisioner,
        )
        .await?;

        match self
            .provider
            .wait_for_provisioner(&runpod_key, &command.workspace_id, &pod_id)
            .await
        {
            Ok(()) => {}
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        }

        self.set_provision_step(
            &workspace,
            &mut operation,
            RunpodProvisionStep::TerminateProvisionerPod,
        )
        .await?;
        match self
            .provider
            .terminate_provisioner_pod(&runpod_key, &pod_id)
            .await
        {
            Ok(()) => {}
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        }
        runpod_mut(&mut workspace)?.resources.provisioner_pod_id = None;
        self.set_provision_step(
            &workspace,
            &mut operation,
            RunpodProvisionStep::CreateTemplate,
        )
        .await?;

        let template_id = match self
            .provider
            .create_template(
                &runpod_key,
                CreateTemplate {
                    workspace_id: command.workspace_id.clone(),
                    image_ref: definition.endpoint_image_ref,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.template_id = Some(template_id.clone());
        self.set_provision_step(
            &workspace,
            &mut operation,
            RunpodProvisionStep::CreateEndpoint,
        )
        .await?;

        let endpoint_id = match self
            .provider
            .create_endpoint(
                &runpod_key,
                CreateEndpoint {
                    workspace_id: command.workspace_id.clone(),
                    datacenter_id: command.datacenter_id,
                    gpu_id: command.gpu_id,
                    network_volume_id: volume_id,
                    template_id,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut workspace, &mut operation).await?;
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.endpoint_id = Some(endpoint_id);

        mark_ready(&mut workspace)?;
        operation.succeed(OffsetDateTime::now_utc())?;
        self.transitions.save(&workspace, &operation).await?;
        Ok(())
    }

    async fn set_provision_step(
        &self,
        workspace: &Workspace,
        operation: &mut RuntimeOperation,
        step: RunpodProvisionStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Provision(step)),
            OffsetDateTime::now_utc(),
        )?;
        self.transitions
            .save(workspace, operation)
            .await
            .map_err(Into::into)
    }

    async fn set_cleanup_step(
        &self,
        workspace: &Workspace,
        operation: &mut RuntimeOperation,
        step: RunpodCleanupStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(step)),
            OffsetDateTime::now_utc(),
        )?;
        self.transitions
            .save(workspace, operation)
            .await
            .map_err(Into::into)
    }

    async fn fail_transition(
        &self,
        workspace: &mut Workspace,
        operation: &mut RuntimeOperation,
    ) -> Result<(), RunpodRuntimeError> {
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
    use uuid::Uuid;

    use crate::application::runtimes::runpod::{
        test_support::{provision_command, CleanupFakes, ProvisionFakes, RecoveryFakes},
        RunpodRuntimeError, RunpodRuntimeResources,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
            Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationState, RuntimeProgress,
            RuntimeProvider, RuntimeState,
        },
        workspace::Workspace,
    };

    #[crate::diagnostics::diagnostic(root)]
    async fn start_provision(
        fakes: &ProvisionFakes,
    ) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError> {
        fakes.service().start_provision(provision_command()).await
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn start_cleanup(
        fakes: &CleanupFakes,
    ) -> Result<(Workspace, RuntimeOperation), RunpodRuntimeError> {
        fakes
            .service()
            .start_cleanup(fakes.workspace_snapshot())
            .await
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn fail_interrupted(fakes: &RecoveryFakes) -> Result<(), RunpodRuntimeError> {
        fakes
            .service()
            .recover_interrupted(fakes.running_operations())
            .await
    }

    fn runpod_progress(progress: RuntimeProgress) -> RunpodProgress {
        let RuntimeProgress::Runpod(progress) = progress;
        progress
    }

    #[tokio::test]
    async fn start_provision_returns_a_durable_operation_before_provider_work_finishes() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();

        let (workspace, operation) = start_provision(&fakes).await.unwrap();
        let RuntimeProvider::Runpod(runtime) = workspace.runtime.as_ref().unwrap().provider.clone();

        assert_eq!(
            workspace.runtime.as_ref().unwrap().state,
            RuntimeState::Provisioning
        );
        assert_eq!(operation.state, RuntimeOperationState::Running);
        assert_eq!(
            runpod_progress(operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(
            fakes.repository.last_workspace_snapshot(),
            (workspace.clone(), operation.clone())
        );
        assert_eq!(
            fakes.events.events()[..2],
            [
                ApplicationEvent::WorkspaceChanged(Workspace {
                    runtime: Some(Runtime {
                        state: RuntimeState::Provisioning,
                        provider: RuntimeProvider::Runpod(runtime.clone()),
                    }),
                    ..fakes.workspace_snapshot()
                }),
                ApplicationEvent::RuntimeOperationChanged(operation.clone()),
            ]
        );
        assert_eq!(operation.runtime_kind, RuntimeKind::Runpod);

        fakes.provider.wait_until_first_call().await;
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Running
        );

        fakes.provider.release_first_call();
        fakes.events.wait_for_terminal_operation(operation.id).await;
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Succeeded
        );
    }

    #[crate::diagnostics::diagnostic(root)]
    #[tokio::test]
    async fn start_provision_persists_the_active_trace() -> Result<(), RunpodRuntimeError> {
        let trace_id = crate::diagnostics::current_trace_uuid().unwrap();
        let fakes = ProvisionFakes::ready();

        let (_, operation) = fakes.service().start_provision(provision_command()).await?;
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(operation.trace_id, Some(trace_id));
        assert!(fakes
            .repository
            .saved_trace_ids()
            .iter()
            .all(|saved| *saved == Some(trace_id)));
        Ok(())
    }

    #[tokio::test]
    async fn provision_persists_each_current_step_before_the_provider_call() {
        let fakes = ProvisionFakes::ready();

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.calls(),
            vec![
                "create_network_volume",
                "start_provisioner_pod",
                "wait_for_provisioner",
                "terminate_provisioner_pod",
                "create_template",
                "create_endpoint",
            ]
        );
        assert_eq!(
            fakes.repository.running_steps(),
            vec![
                RunpodProvisionStep::CreateNetworkVolume,
                RunpodProvisionStep::StartProvisionerPod,
                RunpodProvisionStep::PollProvisioner,
                RunpodProvisionStep::TerminateProvisionerPod,
                RunpodProvisionStep::CreateTemplate,
                RunpodProvisionStep::CreateEndpoint,
            ]
        );
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Succeeded
        );
        assert_eq!(fakes.events.runtime_operation_event_count(), 7);
        assert_eq!(fakes.events.workspace_event_count(), 7);
    }

    #[tokio::test]
    async fn provider_failures_persist_the_failing_step_and_created_resources() {
        let methods = [
            "create_network_volume",
            "start_provisioner_pod",
            "wait_for_provisioner",
            "terminate_provisioner_pod",
            "create_template",
            "create_endpoint",
        ];
        let cases = [
            (
                "create_network_volume",
                RunpodProvisionStep::CreateNetworkVolume,
                RunpodRuntimeResources::default(),
            ),
            (
                "start_provisioner_pod",
                RunpodProvisionStep::StartProvisionerPod,
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    ..Default::default()
                },
            ),
            (
                "wait_for_provisioner",
                RunpodProvisionStep::PollProvisioner,
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    provisioner_pod_id: Some("pod-1".into()),
                    ..Default::default()
                },
            ),
            (
                "terminate_provisioner_pod",
                RunpodProvisionStep::TerminateProvisionerPod,
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    provisioner_pod_id: Some("pod-1".into()),
                    ..Default::default()
                },
            ),
            (
                "create_template",
                RunpodProvisionStep::CreateTemplate,
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    ..Default::default()
                },
            ),
            (
                "create_endpoint",
                RunpodProvisionStep::CreateEndpoint,
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    template_id: Some("template-1".into()),
                    ..Default::default()
                },
            ),
        ];

        for (index, (method, step, resources)) in cases.into_iter().enumerate() {
            let fakes = ProvisionFakes::ready();
            fakes.provider.fail_once(method);

            let (_, started_operation) = start_provision(&fakes).await.unwrap();
            fakes
                .events
                .wait_for_terminal_operation(started_operation.id)
                .await;

            assert_eq!(fakes.provider.calls(), methods[..=index], "{method}");
            let (workspace, operation) = fakes.repository.last_workspace_snapshot();
            let runtime = workspace.runtime.as_ref().unwrap();
            assert_eq!(runtime.state, RuntimeState::Failed, "{method}");
            let RuntimeProvider::Runpod(provider) = &runtime.provider;
            assert_eq!(provider.resources, resources, "{method}");
            assert_eq!(operation.state, RuntimeOperationState::Failed, "{method}");
            assert_eq!(
                runpod_progress(operation.progress).provision_step(),
                Some(step),
                "{method}"
            );
            let events = fakes.events.events();
            assert_eq!(
                &events[events.len() - 2..],
                [
                    ApplicationEvent::WorkspaceChanged(workspace),
                    ApplicationEvent::RuntimeOperationChanged(operation),
                ],
                "{method}"
            );
        }
    }

    #[tokio::test]
    async fn provision_preflight_failure_does_not_save_emit_or_call_provider() {
        let fakes = ProvisionFakes::ready_without_runpod_credential();

        assert_eq!(
            start_provision(&fakes).await,
            Err(RunpodRuntimeError::CredentialMissing)
        );
        assert!(fakes.repository.saved_states().is_empty());
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn oversized_volume_is_rejected_before_provider_work_or_persistence() {
        let fakes = ProvisionFakes::ready();
        let mut command = provision_command();
        command.volume_size_gb = 4_001;

        assert_eq!(
            fakes.service().start_provision(command).await,
            Err(RunpodRuntimeError::InvalidTransition)
        );
        assert!(fakes.repository.saved_states().is_empty());
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn provision_stops_when_progress_persistence_fails() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();
        fakes.repository.fail_transition_after_initial_commit();

        let (workspace, operation) = start_provision(&fakes).await.unwrap();
        fakes.provider.wait_until_first_call().await;
        fakes.provider.release_first_call();
        fakes.repository.wait_for_failed_transition().await;

        assert_eq!(fakes.provider.calls(), vec!["create_network_volume"]);
        assert_eq!(
            fakes.repository.last_workspace_snapshot(),
            (workspace, operation.clone())
        );
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Running
        );
        assert_eq!(
            runpod_progress(operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(fakes.events.runtime_operation_event_count(), 1);
        assert_eq!(fakes.events.workspace_event_count(), 1);
    }

    #[tokio::test]
    async fn start_cleanup_returns_running_snapshots_and_finishes_in_background() {
        let fakes = CleanupFakes::ready_runtime();
        fakes.provider.block_first_call();

        let (workspace, operation) = start_cleanup(&fakes).await.unwrap();
        let runtime = workspace.runtime.as_ref().unwrap();

        assert_eq!(runtime.state, RuntimeState::CleaningUp);
        assert_eq!(operation.state, RuntimeOperationState::Running);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::WorkspaceChanged(workspace.clone()),
                ApplicationEvent::RuntimeOperationChanged(operation.clone()),
            ]
        );

        fakes.provider.wait_until_first_call().await;
        assert!(!fakes.repository.runtime_was_removed());

        fakes.provider.release_first_call();
        fakes.events.wait_for_terminal_operation(operation.id).await;
        assert!(fakes.repository.runtime_was_removed());
    }

    #[tokio::test]
    async fn cleanup_runs_every_step_and_removes_the_runtime() {
        let fakes = CleanupFakes::ready_runtime();

        let (_, started_operation) = start_cleanup(&fakes).await.unwrap();
        fakes
            .events
            .wait_for_terminal_operation(started_operation.id)
            .await;

        assert_eq!(
            fakes.provider.calls(),
            vec![
                "delete_endpoint",
                "delete_template",
                "terminate_provisioner_pod",
                "delete_network_volume",
            ]
        );
        assert_eq!(
            fakes.repository.running_cleanup_steps(),
            vec![
                RunpodCleanupStep::DeleteEndpoint,
                RunpodCleanupStep::DeleteTemplate,
                RunpodCleanupStep::TerminateProvisionerPod,
                RunpodCleanupStep::DeleteNetworkVolume,
            ]
        );
        assert!(fakes.repository.runtime_was_removed());
        assert_eq!(fakes.events.runtime_operation_event_count(), 5);
        assert_eq!(fakes.events.workspace_event_count(), 5);

        let detached_workspace = fakes.workspace_snapshot();
        let (_, succeeded_operation) = fakes.repository.last_workspace_snapshot();
        let events = fakes.events.events();
        assert_eq!(
            &events[events.len() - 2..],
            [
                ApplicationEvent::WorkspaceChanged(detached_workspace),
                ApplicationEvent::RuntimeOperationChanged(succeeded_operation),
            ]
        );
    }

    #[tokio::test]
    async fn cleanup_skips_absent_resource_ids_but_still_records_each_step() {
        let fakes = CleanupFakes::failed_partial_runtime();

        let (_, operation) = start_cleanup(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(fakes.repository.running_cleanup_steps().len(), 4);
        assert_eq!(fakes.provider.calls(), vec!["delete_network_volume"]);
    }

    #[tokio::test]
    async fn cleanup_without_runtime_is_explicit_not_provisioned() {
        let fakes = CleanupFakes::without_runtime();

        assert_eq!(
            start_cleanup(&fakes).await,
            Err(RunpodRuntimeError::NotProvisioned)
        );
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn cleanup_without_runpod_credential_does_not_start_a_transition() {
        let fakes = CleanupFakes::ready_runtime_without_runpod_credential();

        assert_eq!(
            start_cleanup(&fakes).await,
            Err(RunpodRuntimeError::CredentialMissing)
        );
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
        assert!(fakes.repository.saved_states().is_empty());
        assert_eq!(
            fakes.repository.runtime_state("workspace-1"),
            Some(RuntimeState::Ready)
        );
    }

    #[tokio::test]
    async fn cleanup_failure_retains_the_failing_step_and_remaining_resources() {
        let fakes = CleanupFakes::ready_runtime();
        fakes.provider.fail_once("delete_template");

        let (_, started_operation) = start_cleanup(&fakes).await.unwrap();
        fakes
            .events
            .wait_for_terminal_operation(started_operation.id)
            .await;

        let (workspace, operation) = fakes.repository.last_workspace_snapshot();
        let runtime = workspace.runtime.as_ref().unwrap();
        let RuntimeProvider::Runpod(provider) = &runtime.provider;
        assert_eq!(runtime.state, RuntimeState::Failed);
        assert_eq!(provider.resources.endpoint_id, None);
        assert_eq!(
            provider.resources.template_id.as_deref(),
            Some("template-1")
        );
        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteTemplate)
        );
        let events = fakes.events.events();
        assert_eq!(
            &events[events.len() - 2..],
            [
                ApplicationEvent::WorkspaceChanged(workspace),
                ApplicationEvent::RuntimeOperationChanged(operation),
            ]
        );
    }

    #[tokio::test]
    async fn startup_marks_running_operations_and_runtimes_failed() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();

        fail_interrupted(&fakes).await.unwrap();

        assert_eq!(
            fakes.repository.saved_states(),
            vec![
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
                (RuntimeState::Failed, RuntimeOperationState::Failed),
            ]
        );
        assert_eq!(
            fakes.repository.saved_trace_ids(),
            vec![Some(Uuid::from_u128(2)), Some(Uuid::from_u128(4)), None]
        );
        let events = fakes.events.events();
        assert_eq!(events.len(), 6);
        assert!(matches!(
            &events[0],
            ApplicationEvent::WorkspaceChanged(workspace)
                if workspace.id == "workspace-1"
                    && workspace.runtime.as_ref().unwrap().state == RuntimeState::Failed
        ));
        assert!(matches!(
            &events[1],
            ApplicationEvent::RuntimeOperationChanged(operation)
                if operation.state == RuntimeOperationState::Failed
                    && operation.trace_id == Some(Uuid::from_u128(2))
                    && runpod_progress(operation.progress).provision_step()
                        == Some(RunpodProvisionStep::CreateEndpoint)
        ));
        assert!(matches!(
            &events[2],
            ApplicationEvent::WorkspaceChanged(workspace)
                if workspace.id == "workspace-2"
                    && workspace.runtime.as_ref().unwrap().state == RuntimeState::Failed
        ));
        assert!(matches!(
            &events[3],
            ApplicationEvent::RuntimeOperationChanged(operation)
                if operation.state == RuntimeOperationState::Failed
                    && operation.trace_id == Some(Uuid::from_u128(4))
                    && runpod_progress(operation.progress).cleanup_step()
                        == Some(RunpodCleanupStep::DeleteEndpoint)
        ));
        assert!(matches!(
            &events[5],
            ApplicationEvent::RuntimeOperationChanged(operation)
                if operation.state == RuntimeOperationState::Failed
                    && operation.trace_id.is_none()
        ));
        assert_eq!(fakes.events.runtime_operation_event_count(), 3);
        assert_eq!(fakes.events.workspace_event_count(), 3);
        assert!(fakes.provider.calls().is_empty());
    }

    #[tokio::test]
    async fn recovery_entry_point_marks_one_operation_failed() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();
        let service = fakes.service();
        let operation = fakes.running_operations().remove(0);

        service.recover_interrupted(vec![operation]).await.unwrap();

        assert_eq!(
            fakes.repository.saved_states(),
            vec![(RuntimeState::Failed, RuntimeOperationState::Failed)]
        );
        assert_eq!(
            fakes.repository.saved_trace_ids(),
            vec![Some(Uuid::from_u128(2))]
        );
        assert!(fakes.provider.calls().is_empty());
    }
}
