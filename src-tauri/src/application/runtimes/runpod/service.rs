use std::sync::Arc;

use fastrace::{collector::SpanContext, future::FutureExt, Span};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    events::ApplicationEventSink,
    runtimes::{
        ports::RuntimeOperationRepository, RuntimeContractRequirements, RuntimeKind,
        RuntimeOperation, RuntimeOperationKind, RuntimeProgress, RuntimeTransitionContext,
        WorkflowDefinition,
    },
    secrets::{SecretKind, SecretStore},
    workspace::ports::{WorkflowCatalog, WorkspaceRepository},
};

use super::{
    CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodCleanupStep, RunpodProgress,
    RunpodProvisionStep, RunpodRuntime, RunpodRuntimeCatalog, RunpodRuntimeConfig,
    RunpodRuntimeDefinition, RunpodRuntimeError, RunpodRuntimeProvider, RunpodRuntimeRepository,
    StartProvisionerPod,
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

#[derive(Clone)]
pub struct RunpodRuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    runtimes: Arc<dyn RunpodRuntimeRepository>,
    runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    operations: Arc<dyn RuntimeOperationRepository>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn RunpodRuntimeProvider>,
    transitions: RuntimeTransitionContext<RunpodRuntime, dyn RunpodRuntimeRepository>,
}

pub struct RunpodRuntimeServiceDependencies {
    pub workspaces: Arc<dyn WorkspaceRepository>,
    pub workflows: Arc<dyn WorkflowCatalog>,
    pub runtimes: Arc<dyn RunpodRuntimeRepository>,
    pub runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    pub operations: Arc<dyn RuntimeOperationRepository>,
    pub secrets: Arc<dyn SecretStore>,
    pub provider: Arc<dyn RunpodRuntimeProvider>,
    pub events: Arc<dyn ApplicationEventSink>,
}

impl RunpodRuntimeService {
    pub fn new(dependencies: RunpodRuntimeServiceDependencies) -> Self {
        let transitions = RuntimeTransitionContext::new(
            dependencies.runtimes.clone(),
            dependencies.workspaces.clone(),
            dependencies.events,
        );
        Self {
            workspaces: dependencies.workspaces,
            workflows: dependencies.workflows,
            runtimes: dependencies.runtimes,
            runtime_catalog: dependencies.runtime_catalog,
            operations: dependencies.operations,
            secrets: dependencies.secrets,
            provider: dependencies.provider,
            transitions,
        }
    }

    #[crate::diagnostics::diagnostic]
    pub async fn start_cleanup(
        &self,
        #[diagnostic(show)] workspace_id: &str,
    ) -> Result<(RunpodRuntime, RuntimeOperation), RunpodRuntimeError> {
        let mut runtime = self
            .runtimes
            .get(workspace_id)
            .await?
            .ok_or(RunpodRuntimeError::NotProvisioned)?;
        let runpod_key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RunpodRuntimeError::CredentialMissing)?;
        runtime.begin_cleanup()?;

        let operation = RuntimeOperation::running(
            Uuid::new_v4(),
            workspace_id,
            crate::diagnostics::current_trace_id()
                .expect("diagnostic operation has an active trace"),
            RuntimeOperationKind::Cleanup,
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint)),
            OffsetDateTime::now_utc(),
        );
        self.transitions.save_changed(&runtime, &operation).await?;

        let initial_runtime = runtime.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        let workspace_id = workspace_id.to_owned();
        let parent = SpanContext::current_local_parent()
            .expect("diagnostic operation has an active trace context");
        let runner = Span::root("application.runtimes.runpod.run_cleanup", parent);
        tokio::spawn(
            async move {
                service
                    .run_cleanup(workspace_id, runpod_key, runtime, operation)
                    .await
            }
            .in_span(runner),
        );

        Ok((initial_runtime, initial_operation))
    }

    #[crate::diagnostics::diagnostic(show_error)]
    async fn run_cleanup(
        &self,
        #[diagnostic(show)] _workspace_id: String,
        runpod_key: secrecy::SecretString,
        mut runtime: RunpodRuntime,
        mut operation: RuntimeOperation,
    ) -> Result<(), RunpodRuntimeError> {
        if let Some(id) = runtime.resources.endpoint_id.clone() {
            if let Err(error) = self.provider.delete_endpoint(&runpod_key, &id).await {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
            runtime.resources.endpoint_id = None;
        }
        self.set_cleanup_step(&runtime, &mut operation, RunpodCleanupStep::DeleteTemplate)
            .await?;

        if let Some(id) = runtime.resources.template_id.clone() {
            if let Err(error) = self.provider.delete_template(&runpod_key, &id).await {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
            runtime.resources.template_id = None;
        }
        self.set_cleanup_step(
            &runtime,
            &mut operation,
            RunpodCleanupStep::TerminateProvisionerPod,
        )
        .await?;

        if let Some(id) = runtime.resources.provisioner_pod_id.clone() {
            if let Err(error) = self
                .provider
                .terminate_provisioner_pod(&runpod_key, &id)
                .await
            {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
            runtime.resources.provisioner_pod_id = None;
        }
        self.set_cleanup_step(
            &runtime,
            &mut operation,
            RunpodCleanupStep::DeleteNetworkVolume,
        )
        .await?;

        if let Some(id) = runtime.resources.network_volume_id.clone() {
            if let Err(error) = self.provider.delete_network_volume(&runpod_key, &id).await {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
            runtime.resources.network_volume_id = None;
        }

        operation.succeed(OffsetDateTime::now_utc())?;
        self.transitions.save_deleted(&runtime, &operation).await?;
        Ok(())
    }

    #[crate::diagnostics::diagnostic]
    pub async fn fail_interrupted(&self) -> Result<(), RunpodRuntimeError> {
        for operation in self.operations.running().await? {
            let recovery = Span::root(
                "application.runtimes.runpod.recover_interrupted",
                SpanContext::new(operation.trace_id, fastrace::collector::SpanId::default()),
            );
            async {
                let mut operation = operation;
                let RuntimeProgress::Runpod(_) = operation.progress;
                let mut runtime = self
                    .runtimes
                    .get(&operation.workspace_id)
                    .await?
                    .ok_or(RunpodRuntimeError::NotProvisioned)?;
                runtime.mark_failed()?;
                operation.fail(OffsetDateTime::now_utc())?;
                self.transitions.save_changed(&runtime, &operation).await?;
                Ok::<(), RunpodRuntimeError>(())
            }
            .in_span(recovery)
            .await?;
        }
        Ok(())
    }

    #[crate::diagnostics::diagnostic]
    pub async fn start_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRunpodRuntime,
    ) -> Result<(RunpodRuntime, RuntimeOperation), RunpodRuntimeError> {
        let workspace = self
            .workspaces
            .get(&command.workspace_id)
            .await
            .map_err(|_| RunpodRuntimeError::PersistenceUnavailable)?
            .ok_or(RunpodRuntimeError::WorkspaceNotFound)?;

        if workspace.runtime == Some(RuntimeKind::Runpod) {
            let mut runtime = self
                .runtimes
                .get(&command.workspace_id)
                .await?
                .ok_or(RunpodRuntimeError::NotProvisioned)?;
            runtime.begin_provision()?;
        }

        if self.operations.has_running(&command.workspace_id).await? {
            return Err(RunpodRuntimeError::OperationInProgress);
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

        let runtime = RunpodRuntime::new_provisioning(
            command.workspace_id.clone(),
            RunpodRuntimeConfig {
                datacenter_id: command.datacenter_id.clone(),
                gpu_id: command.gpu_id.clone(),
                volume_size_gb: command.volume_size_gb,
            },
        );
        let operation = RuntimeOperation::running(
            Uuid::new_v4(),
            &command.workspace_id,
            crate::diagnostics::current_trace_id()
                .expect("diagnostic operation has an active trace"),
            RuntimeOperationKind::Provision,
            RuntimeProgress::Runpod(RunpodProgress::Provision(
                RunpodProvisionStep::CreateNetworkVolume,
            )),
            OffsetDateTime::now_utc(),
        );
        self.transitions.save_attached(&runtime, &operation).await?;

        let initial_runtime = runtime.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        let parent = SpanContext::current_local_parent()
            .expect("diagnostic operation has an active trace context");
        let runner = Span::root("application.runtimes.runpod.run_provision", parent);
        tokio::spawn(
            async move {
                service
                    .run_provision(
                        command,
                        definition,
                        workflow,
                        runpod_key,
                        hugging_face_api_key,
                        runtime,
                        operation,
                    )
                    .await
            }
            .in_span(runner),
        );

        Ok((initial_runtime, initial_operation))
    }

    #[allow(clippy::too_many_arguments)]
    #[crate::diagnostics::diagnostic(show_error)]
    async fn run_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRunpodRuntime,
        definition: RunpodRuntimeDefinition,
        workflow: WorkflowDefinition,
        runpod_key: secrecy::SecretString,
        hugging_face_api_key: Option<secrecy::SecretString>,
        mut runtime: RunpodRuntime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        };
        runtime.resources.network_volume_id = Some(volume_id.clone());
        self.set_provision_step(
            &runtime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        };
        runtime.resources.provisioner_pod_id = Some(pod_id.clone());
        self.set_provision_step(
            &runtime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        }

        self.set_provision_step(
            &runtime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        }
        runtime.resources.provisioner_pod_id = None;
        self.set_provision_step(
            &runtime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        };
        runtime.resources.template_id = Some(template_id.clone());
        self.set_provision_step(
            &runtime,
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
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(error.into());
            }
        };
        runtime.resources.endpoint_id = Some(endpoint_id);

        runtime.mark_ready()?;
        operation.succeed(OffsetDateTime::now_utc())?;
        self.transitions.save_changed(&runtime, &operation).await?;
        Ok(())
    }

    async fn set_provision_step(
        &self,
        runtime: &RunpodRuntime,
        operation: &mut RuntimeOperation,
        step: RunpodProvisionStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Provision(step)),
            OffsetDateTime::now_utc(),
        )?;
        self.transitions
            .save_changed(runtime, operation)
            .await
            .map_err(Into::into)
    }

    async fn set_cleanup_step(
        &self,
        runtime: &RunpodRuntime,
        operation: &mut RuntimeOperation,
        step: RunpodCleanupStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(step)),
            OffsetDateTime::now_utc(),
        )?;
        self.transitions
            .save_changed(runtime, operation)
            .await
            .map_err(Into::into)
    }

    async fn fail_transition(
        &self,
        runtime: &mut RunpodRuntime,
        operation: &mut RuntimeOperation,
    ) -> Result<(), RunpodRuntimeError> {
        runtime.mark_failed()?;
        operation.fail(OffsetDateTime::now_utc())?;
        self.transitions
            .save_changed(runtime, operation)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use fastrace::collector::TraceId;

    use crate::application::runtimes::runpod::{
        test_support::{provision_command, CleanupFakes, ProvisionFakes, RecoveryFakes},
        RunpodRuntime, RunpodRuntimeError, RunpodRuntimeResources, RunpodRuntimeState,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            runpod::{RunpodCleanupStep, RunpodProgress, RunpodProvisionStep},
            Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationState, RuntimeProgress,
        },
    };

    #[crate::diagnostics::diagnostic(root)]
    async fn start_provision(
        fakes: &ProvisionFakes,
    ) -> Result<(RunpodRuntime, RuntimeOperation), RunpodRuntimeError> {
        fakes.service().start_provision(provision_command()).await
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn start_cleanup(
        fakes: &CleanupFakes,
    ) -> Result<(RunpodRuntime, RuntimeOperation), RunpodRuntimeError> {
        fakes.service().start_cleanup("workspace-1").await
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn fail_interrupted(fakes: &RecoveryFakes) -> Result<(), RunpodRuntimeError> {
        fakes.service().fail_interrupted().await
    }

    fn runpod_progress(progress: RuntimeProgress) -> RunpodProgress {
        let RuntimeProgress::Runpod(progress) = progress;
        progress
    }

    #[tokio::test]
    async fn start_provision_returns_a_durable_operation_before_provider_work_finishes() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();

        let (runtime, operation) = start_provision(&fakes).await.unwrap();

        assert_eq!(runtime.state, RunpodRuntimeState::Provisioning);
        assert_eq!(operation.state, RuntimeOperationState::Running);
        assert_eq!(
            runpod_progress(operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(
            fakes.repository.last_snapshot(),
            (runtime.clone(), operation.clone())
        );
        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::WorkspaceChanged(fakes.workspace_snapshot()),
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime.clone())),
                ApplicationEvent::RuntimeOperationChanged(operation.clone()),
            ]
        );

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
        let trace_id = crate::diagnostics::current_trace_id().unwrap();
        let fakes = ProvisionFakes::ready();

        let (_, operation) = fakes
            .service()
            .start_provision(provision_command())
            .await
            .unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert!(fakes
            .repository
            .saved_trace_ids()
            .iter()
            .all(|saved_trace_id| *saved_trace_id == trace_id));
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
        assert_eq!(fakes.events.runtime_changed_count(), 7);
        assert_eq!(fakes.events.runtime_operation_event_count(), 7);
        assert_eq!(fakes.events.workspace_event_count(), 1);
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
            let (runtime, operation) = fakes.repository.last_snapshot();
            assert_eq!(runtime.state, RunpodRuntimeState::Failed, "{method}");
            assert_eq!(runtime.resources, resources, "{method}");
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
                    ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime)),
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
    async fn provision_stops_when_progress_persistence_fails() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();
        fakes.repository.fail_transition_after_initial_commit();

        let (runtime, operation) = start_provision(&fakes).await.unwrap();
        fakes.provider.wait_until_first_call().await;
        fakes.provider.release_first_call();
        fakes.repository.wait_for_failed_transition().await;

        assert_eq!(fakes.provider.calls(), vec!["create_network_volume"]);
        assert_eq!(
            fakes.repository.last_snapshot(),
            (runtime, operation.clone())
        );
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Running
        );
        assert_eq!(
            runpod_progress(operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(fakes.events.runtime_changed_count(), 1);
        assert_eq!(fakes.events.runtime_operation_event_count(), 1);
        assert_eq!(fakes.events.workspace_event_count(), 1);
    }

    #[tokio::test]
    async fn start_cleanup_returns_running_snapshots_and_finishes_in_background() {
        let fakes = CleanupFakes::ready_runtime();
        fakes.provider.block_first_call();

        let (runtime, operation) = start_cleanup(&fakes).await.unwrap();

        assert_eq!(runtime.state, RunpodRuntimeState::CleaningUp);
        assert_eq!(operation.state, RuntimeOperationState::Running);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime.clone())),
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
        assert_eq!(fakes.events.runtime_changed_count(), 4);
        assert_eq!(fakes.events.runtime_deleted_count(), 1);
        assert_eq!(fakes.events.runtime_operation_event_count(), 5);
        assert_eq!(fakes.events.workspace_event_count(), 1);

        let detached_workspace = fakes.workspace_snapshot();
        let (_, succeeded_operation) = fakes.repository.last_snapshot();
        let events = fakes.events.events();
        assert_eq!(
            &events[events.len() - 3..],
            [
                ApplicationEvent::WorkspaceChanged(detached_workspace),
                ApplicationEvent::RuntimeDeleted {
                    workspace_id: "workspace-1".into(),
                    kind: RuntimeKind::Runpod,
                },
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
            Some(RunpodRuntimeState::Ready)
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

        let (runtime, operation) = fakes.repository.last_snapshot();
        assert_eq!(runtime.state, RunpodRuntimeState::Failed);
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template-1"));
        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteTemplate)
        );
        let events = fakes.events.events();
        assert_eq!(
            &events[events.len() - 2..],
            [
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime)),
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
                (RunpodRuntimeState::Failed, RuntimeOperationState::Failed),
                (RunpodRuntimeState::Failed, RuntimeOperationState::Failed),
            ]
        );
        assert_eq!(
            fakes.repository.saved_trace_ids(),
            vec![TraceId(2), TraceId(4)]
        );
        let events = fakes.events.events();
        assert_eq!(events.len(), 4);
        assert!(matches!(
            &events[0],
            ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime))
                if runtime.workspace_id == "workspace-1"
                    && runtime.state == RunpodRuntimeState::Failed
        ));
        assert!(matches!(
            &events[1],
            ApplicationEvent::RuntimeOperationChanged(operation)
                if operation.state == RuntimeOperationState::Failed
                    && operation.trace_id == TraceId(2)
                    && runpod_progress(operation.progress).provision_step()
                        == Some(RunpodProvisionStep::CreateEndpoint)
        ));
        assert!(matches!(
            &events[2],
            ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime))
                if runtime.workspace_id == "workspace-2"
                    && runtime.state == RunpodRuntimeState::Failed
        ));
        assert!(matches!(
            &events[3],
            ApplicationEvent::RuntimeOperationChanged(operation)
                if operation.state == RuntimeOperationState::Failed
                    && operation.trace_id == TraceId(4)
                    && runpod_progress(operation.progress).cleanup_step()
                        == Some(RunpodCleanupStep::DeleteEndpoint)
        ));
        assert_eq!(fakes.events.runtime_changed_count(), 2);
        assert_eq!(fakes.events.runtime_operation_event_count(), 2);
        assert_eq!(fakes.events.workspace_event_count(), 0);
        assert!(fakes.provider.calls().is_empty());
    }
}
