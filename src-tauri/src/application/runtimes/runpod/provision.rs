use std::time::Duration;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    runtimes::{
        Runtime, RuntimeContractRequirements, RuntimeError, RuntimeKind, RuntimeOperation,
        RuntimeOperationKind, RuntimeProgress, RuntimeProvider, RuntimeState, WorkflowDefinition,
    },
    secrets::SecretKind,
    workspace::Workspace,
};

use super::service::{runpod, runpod_mut, RunpodRuntimeService};
use super::{
    resource_name, CreateEndpoint, CreateNetworkVolume, CreateTemplate, ObserveEndpoint,
    ObserveNetworkVolume, ObserveProvisionerPod, ObserveTemplate, RunpodContractRequirements,
    RunpodProgress, RunpodProvisionStep, RunpodResourceKind, RunpodResourceObservation,
    RunpodRuntime, RunpodRuntimeConfig, RunpodRuntimeDefinition, RunpodRuntimeProviderError,
    StartProvisionerPod, RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
};

const PROVISION_DEADLINE: Duration = Duration::from_secs(2 * 60 * 60);

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

fn provision_conflict(state: RuntimeState) -> RuntimeError {
    match state {
        RuntimeState::Ready => RuntimeError::AlreadyProvisioned,
        RuntimeState::Failed => RuntimeError::RuntimeFailed,
        RuntimeState::Provisioning | RuntimeState::CleaningUp => RuntimeError::OperationInProgress,
    }
}

fn mark_ready(workspace: &mut Workspace) -> Result<(), RuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    if runtime.state != RuntimeState::Provisioning {
        return Err(RuntimeError::InvalidTransition);
    }
    runtime.state = RuntimeState::Ready;
    Ok(())
}

fn runpod_requirements(
    requirements: &[RuntimeContractRequirements],
) -> Result<&RunpodContractRequirements, RuntimeError> {
    requirements
        .iter()
        .find_map(RuntimeContractRequirements::as_runpod)
        .ok_or(RuntimeError::CatalogUnavailable)
}

impl RunpodRuntimeService {
    #[crate::diagnostics::diagnostic]
    pub async fn start_provision(
        &self,
        #[diagnostic(show)] command: ProvisionRunpodRuntime,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        if command.volume_size_gb > RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB {
            return Err(RuntimeError::InvalidTransition);
        }
        let mut workspace = self
            .workspaces
            .get(&command.workspace_id)
            .await
            .map_err(|_| RuntimeError::PersistenceUnavailable)?
            .ok_or(RuntimeError::WorkspaceNotFound)?;

        if let Some(runtime) = &workspace.runtime {
            return Err(provision_conflict(runtime.state));
        }

        let workflow = self
            .workflows
            .get(&workspace.workflow.id, &workspace.workflow.revision)
            .await
            .map_err(|_| RuntimeError::CatalogUnavailable)?
            .ok_or(RuntimeError::WorkflowNotFound)?;
        let requirements = runpod_requirements(&workflow.contract_requirements)?;
        let definition = self
            .runtime_catalog
            .resolve(&workflow.runtime_preset_ref, requirements)
            .await?;
        let runpod_key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RuntimeError::CredentialMissing)?;
        let hugging_face_api_key = if workflow.summary.requires_hugging_face_api_key {
            Some(
                self.secrets
                    .get(SecretKind::HuggingFaceApiKey)
                    .await?
                    .ok_or(RuntimeError::CredentialMissing)?,
            )
        } else {
            None
        };

        let operation_id = Uuid::new_v4();
        workspace.runtime = Some(Runtime {
            state: RuntimeState::Provisioning,
            provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
                operation_id,
                RunpodRuntimeConfig {
                    datacenter_id: command.datacenter_id.clone(),
                    gpu_id: command.gpu_id.clone(),
                    volume_size_gb: command.volume_size_gb,
                },
            )),
        });
        let operation = RuntimeOperation::running(
            operation_id,
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
        self.spawn_supervised(
            operation.id,
            PROVISION_DEADLINE,
            self.clone().run_provision(
                command,
                definition,
                workflow,
                runpod_key,
                hugging_face_api_key,
                workspace,
                operation,
            ),
        );

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
    ) -> Result<(), RuntimeError> {
        let provision_operation_id = runpod(&workspace)?.provision_operation_id;
        let volume_name = resource_name(
            &command.workspace_id,
            provision_operation_id,
            RunpodResourceKind::NetworkVolume,
        );
        let volume_id = match self
            .provider
            .create_network_volume(
                &runpod_key,
                CreateNetworkVolume {
                    name: volume_name.clone(),
                    datacenter_id: command.datacenter_id.clone(),
                    size_gb: command.volume_size_gb,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(RunpodRuntimeProviderError::CreateOutcomeUnknown) => {
                match self
                    .provider
                    .observe_network_volume(
                        &runpod_key,
                        ObserveNetworkVolume {
                            name: volume_name,
                            datacenter_id: command.datacenter_id.clone(),
                            size_gb: command.volume_size_gb,
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Found(id)) => id,
                    Ok(RunpodResourceObservation::Absent) => {
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        for id in ids {
                            let _ = self.provider.delete_network_volume(&runpod_key, &id).await;
                        }
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Err(error) => {
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.network_volume_id = Some(volume_id.clone());
        if let Err(error) = self
            .set_provision_step(
                &workspace,
                &mut operation,
                RunpodProvisionStep::StartProvisionerPod,
            )
            .await
        {
            let _ = self
                .provider
                .delete_network_volume(&runpod_key, &volume_id)
                .await;
            return Err(error);
        }

        let pod_name = resource_name(
            &command.workspace_id,
            provision_operation_id,
            RunpodResourceKind::ProvisionerPod,
        );
        let pod_id = match self
            .provider
            .start_provisioner_pod(
                &runpod_key,
                StartProvisionerPod {
                    workspace_id: command.workspace_id.clone(),
                    name: pod_name.clone(),
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
            Err(RunpodRuntimeProviderError::CreateOutcomeUnknown) => {
                match self
                    .provider
                    .observe_provisioner_pod(
                        &runpod_key,
                        ObserveProvisionerPod {
                            name: pod_name,
                            network_volume_id: volume_id.clone(),
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Found(id)) => id,
                    Ok(RunpodResourceObservation::Absent) => {
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        for id in ids {
                            let _ = self
                                .provider
                                .terminate_provisioner_pod(&runpod_key, &id)
                                .await;
                        }
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Err(error) => {
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.provisioner_pod_id = Some(pod_id.clone());
        if let Err(error) = self
            .set_provision_step(
                &workspace,
                &mut operation,
                RunpodProvisionStep::PollProvisioner,
            )
            .await
        {
            let _ = self
                .provider
                .terminate_provisioner_pod(&runpod_key, &pod_id)
                .await;
            return Err(error);
        }

        match self
            .provider
            .wait_for_provisioner(&runpod_key, &command.workspace_id, &pod_id)
            .await
        {
            Ok(()) => {}
            Err(error) => {
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

        let template_name = resource_name(
            &command.workspace_id,
            provision_operation_id,
            RunpodResourceKind::Template,
        );
        let template_id = match self
            .provider
            .create_template(
                &runpod_key,
                CreateTemplate {
                    name: template_name.clone(),
                    image_ref: definition.endpoint_image_ref,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(RunpodRuntimeProviderError::CreateOutcomeUnknown) => {
                match self
                    .provider
                    .observe_template(
                        &runpod_key,
                        ObserveTemplate {
                            name: template_name,
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Found(id)) => id,
                    Ok(RunpodResourceObservation::Absent) => {
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        for id in ids {
                            let _ = self.provider.delete_template(&runpod_key, &id).await;
                        }
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Err(error) => {
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.template_id = Some(template_id.clone());
        if let Err(error) = self
            .set_provision_step(
                &workspace,
                &mut operation,
                RunpodProvisionStep::CreateEndpoint,
            )
            .await
        {
            let _ = self
                .provider
                .delete_template(&runpod_key, &template_id)
                .await;
            return Err(error);
        }

        let endpoint_name = resource_name(
            &command.workspace_id,
            provision_operation_id,
            RunpodResourceKind::Endpoint,
        );
        let endpoint_id = match self
            .provider
            .create_endpoint(
                &runpod_key,
                CreateEndpoint {
                    name: endpoint_name.clone(),
                    datacenter_id: command.datacenter_id.clone(),
                    gpu_id: command.gpu_id.clone(),
                    network_volume_id: volume_id.clone(),
                    template_id: template_id.clone(),
                },
            )
            .await
        {
            Ok(value) => value,
            Err(RunpodRuntimeProviderError::CreateOutcomeUnknown) => {
                match self
                    .provider
                    .observe_endpoint(
                        &runpod_key,
                        ObserveEndpoint {
                            name: endpoint_name,
                            gpu_id: command.gpu_id,
                            network_volume_id: volume_id,
                            template_id,
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Found(id)) => id,
                    Ok(RunpodResourceObservation::Absent) => {
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        for id in ids {
                            let _ = self.provider.delete_endpoint(&runpod_key, &id).await;
                        }
                        return Err(RuntimeError::ProviderUnavailable);
                    }
                    Err(error) => {
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                return Err(error.into());
            }
        };
        runpod_mut(&mut workspace)?.resources.endpoint_id = Some(endpoint_id.clone());

        mark_ready(&mut workspace)?;
        operation.succeed(OffsetDateTime::now_utc())?;
        if let Err(error) = self.transitions.save(&workspace, &operation).await {
            let _ = self
                .provider
                .delete_endpoint(&runpod_key, &endpoint_id)
                .await;
            return Err(error.into());
        }
        Ok(())
    }

    async fn set_provision_step(
        &self,
        workspace: &Workspace,
        operation: &mut RuntimeOperation,
        step: RunpodProvisionStep,
    ) -> Result<(), RuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Provision(step)),
            OffsetDateTime::now_utc(),
        )?;
        self.transitions
            .save(workspace, operation)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::application::runtimes::runpod::{
        test_support::{provision_command, runpod_progress, yield_until, ProvisionFakes},
        RunpodContractRequirements, RunpodProgress, RunpodProvisionStep, RunpodResourceKind,
        RunpodResourceObservation, RunpodRuntimeProviderError, RunpodRuntimeResources,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            ports::RuntimeTransitionRepository, CatalogRef, Runtime, RuntimeContractRequirements,
            RuntimeError, RuntimeKind, RuntimeOperation, RuntimeOperationState, RuntimeProgress,
            RuntimeProvider, RuntimeState,
        },
        workspace::Workspace,
    };
    use time::OffsetDateTime;

    use super::PROVISION_DEADLINE;

    #[test]
    fn runpod_requirement_lookup_rejects_a_missing_requirement() {
        let expected = RunpodContractRequirements {
            provisioner_contract_ref: CatalogRef::new("provisioner", "1"),
            endpoint_contract_ref: CatalogRef::new("endpoint", "1"),
        };
        let requirements = vec![RuntimeContractRequirements::Runpod(expected.clone())];

        assert_eq!(super::runpod_requirements(&requirements), Ok(&expected));
        assert_eq!(
            super::runpod_requirements(&[]),
            Err(RuntimeError::CatalogUnavailable)
        );
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn start_provision(
        fakes: &ProvisionFakes,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        fakes.service().start_provision(provision_command()).await
    }

    async fn wait_for_deletion_attempts(fakes: &ProvisionFakes, count: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while fakes.provider.deletion_attempts().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn start_provision_returns_a_durable_operation_before_provider_work_finishes() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();

        let (workspace, operation) = start_provision(&fakes).await.unwrap();
        let RuntimeProvider::Runpod(runtime) = workspace.runtime.as_ref().unwrap().provider.clone();

        assert_eq!(runtime.provision_operation_id, operation.id);
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
    async fn start_provision_persists_the_active_trace() -> Result<(), RuntimeError> {
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
    async fn provider_error_is_terminalized_from_durable_state() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();
        fakes.provider.fail_once("create_network_volume");

        let (mut durable_workspace, mut durable_operation) = start_provision(&fakes).await.unwrap();
        fakes.provider.wait_until_first_call().await;
        durable_workspace
            .runtime
            .as_mut()
            .unwrap()
            .provider
            .as_runpod_mut()
            .unwrap()
            .resources
            .network_volume_id = Some("durable-volume".into());
        durable_operation
            .set_progress(
                RuntimeProgress::Runpod(RunpodProgress::Provision(
                    RunpodProvisionStep::StartProvisionerPod,
                )),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        fakes
            .repository
            .save_transition(&durable_workspace, &durable_operation)
            .await
            .unwrap();

        fakes.provider.release_first_call();
        yield_until(|| fakes.events.has_terminal_operation(durable_operation.id)).await;

        let (workspace, operation) = fakes.repository.last_workspace_snapshot();
        let runtime = workspace.runtime.unwrap();
        assert_eq!(runtime.state, RuntimeState::Failed);
        assert_eq!(
            runtime
                .provider
                .as_runpod()
                .unwrap()
                .resources
                .network_volume_id
                .as_deref(),
            Some("durable-volume")
        );
        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(operation.progress).provision_step(),
            Some(RunpodProvisionStep::StartProvisionerPod)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn provision_deadline_aborts_the_body_and_persists_failure() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.block_first_call();

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.provider.wait_until_first_call().await;
        tokio::time::advance(PROVISION_DEADLINE).await;
        yield_until(|| fakes.events.has_terminal_operation(operation.id)).await;

        assert!(fakes.provider.first_call_was_cancelled());
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Failed
        );
        assert_eq!(
            fakes.repository.runtime_state("workspace-1"),
            Some(RuntimeState::Failed)
        );
    }

    #[tokio::test]
    async fn provider_panic_is_observed_and_persisted_as_failure() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.panic_once("create_network_volume");

        let (_, operation) = start_provision(&fakes).await.unwrap();
        yield_until(|| fakes.events.has_terminal_operation(operation.id)).await;

        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Failed
        );
        assert_eq!(
            fakes.repository.runtime_state("workspace-1"),
            Some(RuntimeState::Failed)
        );
    }

    #[tokio::test]
    async fn terminal_persistence_failure_leaves_the_operation_running_for_recovery() {
        let fakes = ProvisionFakes::ready();
        fakes.repository.fail_all_transitions_after_initial_commit();
        fakes.provider.panic_once("create_network_volume");

        let (_, operation) = start_provision(&fakes).await.unwrap();
        yield_until(|| fakes.repository.failed_write_was_attempted()).await;

        assert!(fakes.repository.failed_write_was_attempted());
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Running
        );
        assert_eq!(
            fakes.repository.runtime_state("workspace-1"),
            Some(RuntimeState::Provisioning)
        );
        assert!(!fakes.events.has_terminal_operation(operation.id));
    }

    #[tokio::test]
    async fn unknown_create_adopts_one_observed_resource_before_advancing() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.fail_once_with(
            "create_network_volume",
            RunpodRuntimeProviderError::CreateOutcomeUnknown,
        );
        fakes.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Found("observed-volume".into()),
        );

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.calls()[..3],
            [
                "create_network_volume",
                "observe_network_volume",
                "start_provisioner_pod",
            ]
        );
        assert_eq!(
            fakes
                .repository
                .resources_at_provision_step(RunpodProvisionStep::StartProvisionerPod)
                .unwrap()
                .network_volume_id
                .as_deref(),
            Some("observed-volume")
        );
        assert_eq!(
            fakes
                .repository
                .last_snapshot()
                .0
                .resources
                .network_volume_id
                .as_deref(),
            Some("observed-volume")
        );
    }

    #[tokio::test]
    async fn unknown_create_absence_fails_without_starting_the_next_acquisition() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.fail_once_with(
            "create_network_volume",
            RunpodRuntimeProviderError::CreateOutcomeUnknown,
        );
        fakes.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Absent,
        );

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.calls(),
            vec!["create_network_volume", "observe_network_volume"]
        );
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Failed
        );
        assert!(fakes.provider.deletion_attempts().is_empty());
    }

    #[tokio::test]
    async fn unknown_create_ambiguity_compensates_every_match_and_selects_none() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.fail_once_with(
            "create_network_volume",
            RunpodRuntimeProviderError::CreateOutcomeUnknown,
        );
        fakes.provider.fail_once_with(
            "delete_network_volume",
            RunpodRuntimeProviderError::Unavailable,
        );
        fakes.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Ambiguous(vec!["volume-a".into(), "volume-b".into()]),
        );

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.deletion_attempts(),
            vec![
                (RunpodResourceKind::NetworkVolume, "volume-a".into()),
                (RunpodResourceKind::NetworkVolume, "volume-b".into()),
            ]
        );
        assert_eq!(
            fakes
                .repository
                .last_snapshot()
                .0
                .resources
                .network_volume_id,
            None
        );
        assert!(!fakes.provider.calls().contains(&"start_provisioner_pod"));
    }

    #[tokio::test]
    async fn unknown_create_observation_failure_is_not_treated_as_absence() {
        let fakes = ProvisionFakes::ready();
        fakes.provider.fail_once_with(
            "create_network_volume",
            RunpodRuntimeProviderError::CreateOutcomeUnknown,
        );
        fakes.provider.fail_once_with(
            "observe_network_volume",
            RunpodRuntimeProviderError::ObserveUnavailable,
        );

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.calls(),
            vec!["create_network_volume", "observe_network_volume"]
        );
        assert_eq!(
            fakes.repository.last_operation_state(),
            RuntimeOperationState::Failed
        );
        assert!(fakes.provider.deletion_attempts().is_empty());
    }

    #[tokio::test]
    async fn created_resource_is_compensated_when_its_id_cannot_be_persisted() {
        let fakes = ProvisionFakes::ready();
        fakes.repository.fail_transition_after_initial_commit();

        let (_, operation) = start_provision(&fakes).await.unwrap();
        fakes.repository.wait_for_failed_transition().await;
        wait_for_deletion_attempts(&fakes, 1).await;
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.deletion_attempts(),
            vec![(RunpodResourceKind::NetworkVolume, "volume-1".into())]
        );
        assert!(!fakes.provider.calls().contains(&"start_provisioner_pod"));
        let (_, durable_operation) = fakes.repository.last_workspace_snapshot();
        assert_eq!(durable_operation.id, operation.id);
        assert_eq!(durable_operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(durable_operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
    }

    #[tokio::test]
    async fn provision_preflight_failure_does_not_save_emit_or_call_provider() {
        let fakes = ProvisionFakes::ready_without_runpod_credential();

        assert_eq!(
            start_provision(&fakes).await,
            Err(RuntimeError::CredentialMissing)
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
            Err(RuntimeError::InvalidTransition)
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
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            fakes.provider.calls(),
            vec!["create_network_volume", "delete_network_volume"]
        );
        let (failed_workspace, failed_operation) = fakes.repository.last_workspace_snapshot();
        assert_eq!(failed_workspace.id, workspace.id);
        assert_eq!(
            failed_workspace.runtime.unwrap().state,
            RuntimeState::Failed
        );
        assert_eq!(failed_operation.id, operation.id);
        assert_eq!(failed_operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(failed_operation.progress).provision_step(),
            Some(RunpodProvisionStep::CreateNetworkVolume)
        );
        assert_eq!(fakes.events.runtime_operation_event_count(), 2);
        assert_eq!(fakes.events.workspace_event_count(), 2);
    }
}
