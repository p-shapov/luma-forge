use std::sync::Arc;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    catalog::{RunpodRuntimeDefinition, WorkflowDefinition},
    events::ApplicationEventSink,
    lifecycle::{
        background::LifecycleBackgroundRunner,
        ports::LifecycleOperationRepository,
        progress::runpod::{RunpodCleanupStep, RunpodProvisionStep},
        LifecycleOperation, LifecycleProgress,
    },
    runtimes::RuntimeTransitionContext,
    secrets::{SecretKind, SecretStore},
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository},
        RuntimeKind,
    },
};

use super::{
    CreateEndpoint, CreateNetworkVolume, CreateTemplate, RunpodRuntime, RunpodRuntimeCatalog,
    RunpodRuntimeConfig, RunpodRuntimeError, RunpodRuntimeProvider, RunpodRuntimeRepository,
    StartProvisionerPod,
};

pub struct ProvisionRunpodRuntime {
    pub workspace_id: String,
    pub datacenter_id: String,
    pub gpu_id: String,
    pub volume_size_gb: u64,
}

#[derive(Clone)]
pub struct RunpodRuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    runtimes: Arc<dyn RunpodRuntimeRepository>,
    runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    lifecycle: Arc<dyn LifecycleOperationRepository>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn RunpodRuntimeProvider>,
    transitions: RuntimeTransitionContext<RunpodRuntime, dyn RunpodRuntimeRepository>,
    background: LifecycleBackgroundRunner,
}

pub struct RunpodRuntimeServiceDependencies {
    pub workspaces: Arc<dyn WorkspaceRepository>,
    pub workflows: Arc<dyn WorkflowCatalog>,
    pub runtimes: Arc<dyn RunpodRuntimeRepository>,
    pub runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    pub lifecycle: Arc<dyn LifecycleOperationRepository>,
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
            lifecycle: dependencies.lifecycle,
            secrets: dependencies.secrets,
            provider: dependencies.provider,
            transitions,
            background: LifecycleBackgroundRunner,
        }
    }

    pub async fn start_cleanup(
        &self,
        workspace_id: &str,
    ) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError> {
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

        let operation = LifecycleOperation::runpod_cleanup(
            Uuid::new_v4(),
            workspace_id,
            Uuid::new_v4(),
            RunpodCleanupStep::DeleteEndpoint,
            OffsetDateTime::now_utc(),
        );
        self.transitions.save_changed(&runtime, &operation).await?;

        let initial_runtime = runtime.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        let workspace_id = workspace_id.to_owned();
        self.background.spawn(async move {
            service
                .run_cleanup(workspace_id, runpod_key, runtime, operation)
                .await;
        });

        Ok((initial_runtime, initial_operation))
    }

    async fn run_cleanup(
        &self,
        _workspace_id: String,
        runpod_key: secrecy::SecretString,
        mut runtime: RunpodRuntime,
        mut operation: LifecycleOperation,
    ) {
        if let Some(id) = runtime.resources.endpoint_id.clone() {
            if self
                .provider
                .delete_endpoint(&runpod_key, &id)
                .await
                .is_err()
            {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
            runtime.resources.endpoint_id = None;
        }
        if self
            .set_cleanup_step(&runtime, &mut operation, RunpodCleanupStep::DeleteTemplate)
            .await
            .is_err()
        {
            return;
        }

        if let Some(id) = runtime.resources.template_id.clone() {
            if self
                .provider
                .delete_template(&runpod_key, &id)
                .await
                .is_err()
            {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
            runtime.resources.template_id = None;
        }
        if self
            .set_cleanup_step(
                &runtime,
                &mut operation,
                RunpodCleanupStep::TerminateProvisionerPod,
            )
            .await
            .is_err()
        {
            return;
        }

        if let Some(id) = runtime.resources.provisioner_pod_id.clone() {
            if self
                .provider
                .terminate_provisioner_pod(&runpod_key, &id)
                .await
                .is_err()
            {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
            runtime.resources.provisioner_pod_id = None;
        }
        if self
            .set_cleanup_step(
                &runtime,
                &mut operation,
                RunpodCleanupStep::DeleteNetworkVolume,
            )
            .await
            .is_err()
        {
            return;
        }

        if let Some(id) = runtime.resources.network_volume_id.clone() {
            if self
                .provider
                .delete_network_volume(&runpod_key, &id)
                .await
                .is_err()
            {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
            runtime.resources.network_volume_id = None;
        }

        if operation.succeed(OffsetDateTime::now_utc()).is_err() {
            return;
        }
        let _ = self.transitions.save_deleted(&runtime, &operation).await;
    }

    pub async fn fail_interrupted(&self) -> Result<(), RunpodRuntimeError> {
        for mut operation in self.lifecycle.running().await? {
            let LifecycleProgress::Runpod(_) = operation.progress;
            let mut runtime = self
                .runtimes
                .get(&operation.workspace_id)
                .await?
                .ok_or(RunpodRuntimeError::NotProvisioned)?;
            runtime.mark_failed()?;
            operation.fail(OffsetDateTime::now_utc())?;
            self.transitions.save_changed(&runtime, &operation).await?;
        }
        Ok(())
    }

    pub async fn start_provision(
        &self,
        command: ProvisionRunpodRuntime,
    ) -> Result<(RunpodRuntime, LifecycleOperation), RunpodRuntimeError> {
        let workspace = self
            .workspaces
            .get(&command.workspace_id)
            .await
            .map_err(|_| RunpodRuntimeError::PersistenceUnavailable)?
            .ok_or(RunpodRuntimeError::WorkspaceNotFound)?;

        if workspace.attached_runtime == Some(RuntimeKind::Runpod) {
            let mut runtime = self
                .runtimes
                .get(&command.workspace_id)
                .await?
                .ok_or(RunpodRuntimeError::NotProvisioned)?;
            runtime.begin_provision()?;
        }

        if self.lifecycle.has_running(&command.workspace_id).await? {
            return Err(RunpodRuntimeError::OperationInProgress);
        }

        let workflow = self
            .workflows
            .get(&workspace.workflow.id, &workspace.workflow.revision)
            .await
            .map_err(|_| RunpodRuntimeError::CatalogUnavailable)?
            .ok_or(RunpodRuntimeError::WorkflowNotFound)?;
        let requirements = WorkflowDefinition::runpod_requirements(&workflow.contract_requirements)
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
        let operation = LifecycleOperation::runpod_provision(
            Uuid::new_v4(),
            &command.workspace_id,
            Uuid::new_v4(),
            RunpodProvisionStep::CreateNetworkVolume,
            OffsetDateTime::now_utc(),
        );
        self.transitions.save_attached(&runtime, &operation).await?;

        let initial_runtime = runtime.clone();
        let initial_operation = operation.clone();
        let service = self.clone();
        self.background.spawn(async move {
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
                .await;
        });

        Ok((initial_runtime, initial_operation))
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_provision(
        &self,
        command: ProvisionRunpodRuntime,
        definition: RunpodRuntimeDefinition,
        workflow: WorkflowDefinition,
        runpod_key: secrecy::SecretString,
        hugging_face_api_key: Option<secrecy::SecretString>,
        mut runtime: RunpodRuntime,
        mut operation: LifecycleOperation,
    ) {
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
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        };
        runtime.resources.network_volume_id = Some(volume_id.clone());
        if self
            .set_provision_step(
                &runtime,
                &mut operation,
                RunpodProvisionStep::StartProvisionerPod,
            )
            .await
            .is_err()
        {
            return;
        }

        let pod_id = match self
            .provider
            .start_provisioner_pod(
                &runpod_key,
                StartProvisionerPod {
                    workspace_id: command.workspace_id.clone(),
                    datacenter_id: command.datacenter_id.clone(),
                    network_volume_id: volume_id.clone(),
                    provisioner_image_ref: definition.provisioner_contract.image_ref,
                    required_model_assets: workflow.model_assets,
                    hugging_face_api_key,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        };
        runtime.resources.provisioner_pod_id = Some(pod_id.clone());
        if self
            .set_provision_step(
                &runtime,
                &mut operation,
                RunpodProvisionStep::PollProvisioner,
            )
            .await
            .is_err()
        {
            return;
        }

        match self
            .provider
            .wait_for_provisioner(&runpod_key, &command.workspace_id, &pod_id)
            .await
        {
            Ok(()) => {}
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        }

        if self
            .set_provision_step(
                &runtime,
                &mut operation,
                RunpodProvisionStep::TerminateProvisionerPod,
            )
            .await
            .is_err()
        {
            return;
        }
        match self
            .provider
            .terminate_provisioner_pod(&runpod_key, &pod_id)
            .await
        {
            Ok(()) => {}
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        }
        runtime.resources.provisioner_pod_id = None;
        if self
            .set_provision_step(
                &runtime,
                &mut operation,
                RunpodProvisionStep::CreateTemplate,
            )
            .await
            .is_err()
        {
            return;
        }

        let template_id = match self
            .provider
            .create_template(
                &runpod_key,
                CreateTemplate {
                    workspace_id: command.workspace_id.clone(),
                    image_ref: definition.endpoint_contract.image_ref,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        };
        runtime.resources.template_id = Some(template_id.clone());
        if self
            .set_provision_step(
                &runtime,
                &mut operation,
                RunpodProvisionStep::CreateEndpoint,
            )
            .await
            .is_err()
        {
            return;
        }

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
            Err(_) => {
                let _ = self.fail_transition(&mut runtime, &mut operation).await;
                return;
            }
        };
        runtime.resources.endpoint_id = Some(endpoint_id);

        if runtime.mark_ready().is_err() || operation.succeed(OffsetDateTime::now_utc()).is_err() {
            return;
        }
        let _ = self.transitions.save_changed(&runtime, &operation).await;
    }

    async fn set_provision_step(
        &self,
        runtime: &RunpodRuntime,
        operation: &mut LifecycleOperation,
        step: RunpodProvisionStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_provision_step(step, OffsetDateTime::now_utc())?;
        self.transitions
            .save_changed(runtime, operation)
            .await
            .map_err(Into::into)
    }

    async fn set_cleanup_step(
        &self,
        runtime: &RunpodRuntime,
        operation: &mut LifecycleOperation,
        step: RunpodCleanupStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_cleanup_step(step, OffsetDateTime::now_utc())?;
        self.transitions
            .save_changed(runtime, operation)
            .await
            .map_err(Into::into)
    }

    async fn fail_transition(
        &self,
        runtime: &mut RunpodRuntime,
        operation: &mut LifecycleOperation,
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
    use crate::application::{
        events::ApplicationEvent,
        lifecycle::{
            progress::runpod::{RunpodCleanupStep, RunpodProvisionStep},
            LifecycleOperationState,
        },
        runtimes::Runtime,
    };
    use uuid::Uuid;

    use crate::application::runtimes::runpod::{
        test_support::{provision_command, CleanupFakes, ProvisionFakes, RecoveryFakes},
        RunpodRuntimeError, RunpodRuntimeResources, RunpodRuntimeState,
    };

    #[tokio::test]
    async fn start_provision_returns_a_durable_operation_before_provider_work_finishes() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();

        let (runtime, operation) = fakes
            .service()
            .start_provision(provision_command())
            .await
            .unwrap();

        assert_eq!(runtime.state, RunpodRuntimeState::Provisioning);
        assert_eq!(operation.state, LifecycleOperationState::Running);
        assert_eq!(
            operation.progress.provision_step(),
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
                ApplicationEvent::LifecycleOperationChanged(operation.clone()),
            ]
        );

        fakes.provider.wait_until_first_call().await;
        assert_eq!(
            fakes.repository.last_operation_state(),
            LifecycleOperationState::Running
        );

        fakes.provider.release_first_call();
        fakes.events.wait_for_terminal_operation(operation.id).await;
        assert_eq!(
            fakes.repository.last_operation_state(),
            LifecycleOperationState::Succeeded
        );
    }

    #[tokio::test]
    async fn provision_persists_each_current_step_before_the_provider_call() {
        let fakes = ProvisionFakes::ready();

        let (_, operation) = fakes
            .service()
            .start_provision(provision_command())
            .await
            .unwrap();
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
            LifecycleOperationState::Succeeded
        );
        assert_eq!(fakes.events.runtime_changed_count(), 7);
        assert_eq!(fakes.events.lifecycle_event_count(), 7);
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

            let (_, started_operation) = fakes
                .service()
                .start_provision(provision_command())
                .await
                .unwrap();
            fakes
                .events
                .wait_for_terminal_operation(started_operation.id)
                .await;

            assert_eq!(fakes.provider.calls(), methods[..=index], "{method}");
            let (runtime, operation) = fakes.repository.last_snapshot();
            assert_eq!(runtime.state, RunpodRuntimeState::Failed, "{method}");
            assert_eq!(runtime.resources, resources, "{method}");
            assert_eq!(operation.state, LifecycleOperationState::Failed, "{method}");
            assert_eq!(operation.progress.provision_step(), Some(step), "{method}");
            let events = fakes.events.events();
            assert_eq!(
                &events[events.len() - 2..],
                [
                    ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime)),
                    ApplicationEvent::LifecycleOperationChanged(operation),
                ],
                "{method}"
            );
        }
    }

    #[tokio::test]
    async fn provision_preflight_failure_does_not_save_emit_or_call_provider() {
        let fakes = ProvisionFakes::ready_without_runpod_credential();

        assert_eq!(
            fakes.service().start_provision(provision_command()).await,
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

        let (runtime, operation) = fakes
            .service()
            .start_provision(provision_command())
            .await
            .unwrap();
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
            LifecycleOperationState::Running
        );
        assert_eq!(
            operation.progress.provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(fakes.events.runtime_changed_count(), 1);
        assert_eq!(fakes.events.lifecycle_event_count(), 1);
        assert_eq!(fakes.events.workspace_event_count(), 1);
    }

    #[tokio::test]
    async fn start_cleanup_returns_running_snapshots_and_finishes_in_background() {
        let fakes = CleanupFakes::ready_runtime();
        fakes.provider.block_first_call();

        let (runtime, operation) = fakes.service().start_cleanup("workspace-1").await.unwrap();

        assert_eq!(runtime.state, RunpodRuntimeState::CleaningUp);
        assert_eq!(operation.state, LifecycleOperationState::Running);
        assert_eq!(
            operation.progress.cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
        assert_eq!(
            fakes.events.events(),
            vec![
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime.clone())),
                ApplicationEvent::LifecycleOperationChanged(operation.clone()),
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

        let (_, started_operation) = fakes.service().start_cleanup("workspace-1").await.unwrap();
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
        assert_eq!(fakes.events.lifecycle_event_count(), 5);
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
                    kind: crate::application::workspace::RuntimeKind::Runpod,
                },
                ApplicationEvent::LifecycleOperationChanged(succeeded_operation),
            ]
        );
    }

    #[tokio::test]
    async fn cleanup_skips_absent_resource_ids_but_still_records_each_step() {
        let fakes = CleanupFakes::failed_partial_runtime();

        let (_, operation) = fakes.service().start_cleanup("workspace-1").await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(fakes.repository.running_cleanup_steps().len(), 4);
        assert_eq!(fakes.provider.calls(), vec!["delete_network_volume"]);
    }

    #[tokio::test]
    async fn cleanup_without_runtime_is_explicit_not_provisioned() {
        let fakes = CleanupFakes::without_runtime();

        assert_eq!(
            fakes.service().start_cleanup("workspace-1").await,
            Err(RunpodRuntimeError::NotProvisioned)
        );
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn cleanup_without_runpod_credential_does_not_start_a_transition() {
        let fakes = CleanupFakes::ready_runtime_without_runpod_credential();

        assert_eq!(
            fakes.service().start_cleanup("workspace-1").await,
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

        let (_, started_operation) = fakes.service().start_cleanup("workspace-1").await.unwrap();
        fakes
            .events
            .wait_for_terminal_operation(started_operation.id)
            .await;

        let (runtime, operation) = fakes.repository.last_snapshot();
        assert_eq!(runtime.state, RunpodRuntimeState::Failed);
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template-1"));
        assert_eq!(operation.state, LifecycleOperationState::Failed);
        assert_eq!(
            operation.progress.cleanup_step(),
            Some(RunpodCleanupStep::DeleteTemplate)
        );
        let events = fakes.events.events();
        assert_eq!(
            &events[events.len() - 2..],
            [
                ApplicationEvent::RuntimeChanged(Runtime::Runpod(runtime)),
                ApplicationEvent::LifecycleOperationChanged(operation),
            ]
        );
    }

    #[tokio::test]
    async fn startup_marks_running_operations_and_runtimes_failed() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();

        fakes.service().fail_interrupted().await.unwrap();

        assert_eq!(
            fakes.repository.saved_states(),
            vec![
                (RunpodRuntimeState::Failed, LifecycleOperationState::Failed),
                (RunpodRuntimeState::Failed, LifecycleOperationState::Failed),
            ]
        );
        assert_eq!(
            fakes.repository.saved_trace_ids(),
            vec![Uuid::from_u128(2), Uuid::from_u128(4)]
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
            ApplicationEvent::LifecycleOperationChanged(operation)
                if operation.state == LifecycleOperationState::Failed
                    && operation.trace_id == Uuid::from_u128(2)
                    && operation.progress.provision_step()
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
            ApplicationEvent::LifecycleOperationChanged(operation)
                if operation.state == LifecycleOperationState::Failed
                    && operation.trace_id == Uuid::from_u128(4)
                    && operation.progress.cleanup_step()
                        == Some(RunpodCleanupStep::DeleteEndpoint)
        ));
        assert_eq!(fakes.events.runtime_changed_count(), 2);
        assert_eq!(fakes.events.lifecycle_event_count(), 2);
        assert_eq!(fakes.events.workspace_event_count(), 0);
        assert!(fakes.provider.calls().is_empty());
    }
}
