use std::time::Duration;

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    runtimes::{
        RuntimeError, RuntimeKind, RuntimeOperation, RuntimeOperationKind, RuntimeProgress,
        RuntimeState,
    },
    secrets::SecretKind,
    workspace::Workspace,
};

use super::service::{runpod, runpod_mut, RunpodRuntimeService};
use super::{
    resource_name, ObserveEndpoint, ObserveNetworkVolume, ObserveProvisionerPod, ObserveTemplate,
    RunpodCleanupStep, RunpodProgress, RunpodResourceKind, RunpodResourceObservation,
};

const CLEANUP_DEADLINE: Duration = Duration::from_secs(5 * 60);

fn begin_cleanup(workspace: &mut Workspace) -> Result<(), RuntimeError> {
    runpod(workspace)?;
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    match runtime.state {
        RuntimeState::Ready | RuntimeState::Failed => {
            runtime.state = RuntimeState::CleaningUp;
            Ok(())
        }
        RuntimeState::Provisioning | RuntimeState::CleaningUp => {
            Err(RuntimeError::OperationInProgress)
        }
    }
}

impl RunpodRuntimeService {
    #[luma_diagnostics::diagnostic]
    pub async fn start_cleanup(
        &self,
        #[diagnostic(show)] mut workspace: Workspace,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        let workspace_id = workspace.id.clone();
        let runpod_key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RuntimeError::CredentialMissing)?;
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
        self.spawn_supervised(
            operation.id,
            CLEANUP_DEADLINE,
            self.clone()
                .run_cleanup(workspace_id, runpod_key, workspace, operation),
        );

        Ok((initial_workspace, initial_operation))
    }

    #[luma_diagnostics::diagnostic(detached, show_error)]
    async fn run_cleanup(
        self,
        #[diagnostic(show)] workspace_id: String,
        runpod_key: secrecy::SecretString,
        mut workspace: Workspace,
        mut operation: RuntimeOperation,
    ) -> Result<(), RuntimeError> {
        let endpoint_id = match runpod(&workspace)?.resources.endpoint_id.clone() {
            Some(id) => Some(id),
            None => {
                let runtime = runpod(&workspace)?;
                match (
                    runtime.resources.network_volume_id.clone(),
                    runtime.resources.template_id.clone(),
                ) {
                    (Some(network_volume_id), Some(template_id)) => {
                        match self
                            .provider
                            .observe_endpoint(
                                &runpod_key,
                                ObserveEndpoint {
                                    name: resource_name(
                                        &workspace_id,
                                        runtime.provision_operation_id,
                                        RunpodResourceKind::Endpoint,
                                    ),
                                    gpu_id: runtime.config.gpu_id.clone(),
                                    network_volume_id,
                                    template_id,
                                },
                            )
                            .await
                        {
                            Ok(RunpodResourceObservation::Absent) => None,
                            Ok(RunpodResourceObservation::Found(id)) => {
                                runpod_mut(&mut workspace)?.resources.endpoint_id =
                                    Some(id.clone());
                                self.transitions.save(&workspace, &operation).await?;
                                Some(id)
                            }
                            Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                let mut first_error = None;
                                for id in ids {
                                    if let Err(error) =
                                        self.provider.delete_endpoint(&runpod_key, &id).await
                                    {
                                        first_error.get_or_insert(error);
                                    }
                                }
                                if let Some(error) = first_error {
                                    return Err(error.into());
                                }
                                None
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                    _ => None,
                }
            }
        };
        if let Some(id) = endpoint_id {
            if let Err(error) = self.provider.delete_endpoint(&runpod_key, &id).await {
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

        let template_id = match runpod(&workspace)?.resources.template_id.clone() {
            Some(id) => Some(id),
            None => {
                let runtime = runpod(&workspace)?;
                match self
                    .provider
                    .observe_template(
                        &runpod_key,
                        ObserveTemplate {
                            name: resource_name(
                                &workspace_id,
                                runtime.provision_operation_id,
                                RunpodResourceKind::Template,
                            ),
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Absent) => None,
                    Ok(RunpodResourceObservation::Found(id)) => {
                        runpod_mut(&mut workspace)?.resources.template_id = Some(id.clone());
                        self.transitions.save(&workspace, &operation).await?;
                        Some(id)
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        let mut first_error = None;
                        for id in ids {
                            if let Err(error) =
                                self.provider.delete_template(&runpod_key, &id).await
                            {
                                first_error.get_or_insert(error);
                            }
                        }
                        if let Some(error) = first_error {
                            return Err(error.into());
                        }
                        None
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        if let Some(id) = template_id {
            if let Err(error) = self.provider.delete_template(&runpod_key, &id).await {
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

        let provisioner_pod_id = match runpod(&workspace)?.resources.provisioner_pod_id.clone() {
            Some(id) => Some(id),
            None => {
                let runtime = runpod(&workspace)?;
                match runtime.resources.network_volume_id.clone() {
                    Some(network_volume_id) => {
                        match self
                            .provider
                            .observe_provisioner_pod(
                                &runpod_key,
                                ObserveProvisionerPod {
                                    name: resource_name(
                                        &workspace_id,
                                        runtime.provision_operation_id,
                                        RunpodResourceKind::ProvisionerPod,
                                    ),
                                    network_volume_id,
                                },
                            )
                            .await
                        {
                            Ok(RunpodResourceObservation::Absent) => None,
                            Ok(RunpodResourceObservation::Found(id)) => {
                                runpod_mut(&mut workspace)?.resources.provisioner_pod_id =
                                    Some(id.clone());
                                self.transitions.save(&workspace, &operation).await?;
                                Some(id)
                            }
                            Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                let mut first_error = None;
                                for id in ids {
                                    if let Err(error) = self
                                        .provider
                                        .terminate_provisioner_pod(&runpod_key, &id)
                                        .await
                                    {
                                        first_error.get_or_insert(error);
                                    }
                                }
                                if let Some(error) = first_error {
                                    return Err(error.into());
                                }
                                None
                            }
                            Err(error) => return Err(error.into()),
                        }
                    }
                    None => None,
                }
            }
        };
        if let Some(id) = provisioner_pod_id {
            if let Err(error) = self
                .provider
                .terminate_provisioner_pod(&runpod_key, &id)
                .await
            {
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

        let network_volume_id = match runpod(&workspace)?.resources.network_volume_id.clone() {
            Some(id) => Some(id),
            None => {
                let runtime = runpod(&workspace)?;
                match self
                    .provider
                    .observe_network_volume(
                        &runpod_key,
                        ObserveNetworkVolume {
                            name: resource_name(
                                &workspace_id,
                                runtime.provision_operation_id,
                                RunpodResourceKind::NetworkVolume,
                            ),
                            datacenter_id: runtime.config.datacenter_id.clone(),
                            size_gb: runtime.config.volume_size_gb,
                        },
                    )
                    .await
                {
                    Ok(RunpodResourceObservation::Absent) => None,
                    Ok(RunpodResourceObservation::Found(id)) => {
                        runpod_mut(&mut workspace)?.resources.network_volume_id = Some(id.clone());
                        self.transitions.save(&workspace, &operation).await?;
                        Some(id)
                    }
                    Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                        let mut first_error = None;
                        for id in ids {
                            if let Err(error) =
                                self.provider.delete_network_volume(&runpod_key, &id).await
                            {
                                first_error.get_or_insert(error);
                            }
                        }
                        if let Some(error) = first_error {
                            return Err(error.into());
                        }
                        None
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        if let Some(id) = network_volume_id {
            if let Err(error) = self.provider.delete_network_volume(&runpod_key, &id).await {
                return Err(error.into());
            }
            runpod_mut(&mut workspace)?.resources.network_volume_id = None;
        }

        operation.succeed(OffsetDateTime::now_utc())?;
        workspace.runtime = None;
        self.transitions.save(&workspace, &operation).await?;
        Ok(())
    }

    async fn set_cleanup_step(
        &self,
        workspace: &Workspace,
        operation: &mut RuntimeOperation,
        step: RunpodCleanupStep,
    ) -> Result<(), RuntimeError> {
        operation.set_progress(
            RuntimeProgress::Runpod(RunpodProgress::Cleanup(step)),
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
    use uuid::Uuid;

    use crate::application::runtimes::runpod::{
        test_support::{runpod_progress, yield_until, CleanupFakes},
        RunpodCleanupStep, RunpodResourceKind, RunpodResourceObservation,
        RunpodRuntimeProviderError, RunpodRuntimeResources,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            RuntimeError, RuntimeOperation, RuntimeOperationState, RuntimeProvider, RuntimeState,
        },
        workspace::Workspace,
    };

    use super::CLEANUP_DEADLINE;

    #[luma_diagnostics::diagnostic(root)]
    async fn start_cleanup(
        fakes: &CleanupFakes,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        fakes
            .service()
            .start_cleanup(fakes.workspace_snapshot())
            .await
    }

    fn resource_id(resources: &RunpodRuntimeResources, kind: RunpodResourceKind) -> Option<&str> {
        match kind {
            RunpodResourceKind::NetworkVolume => resources.network_volume_id.as_deref(),
            RunpodResourceKind::ProvisionerPod => resources.provisioner_pod_id.as_deref(),
            RunpodResourceKind::Template => resources.template_id.as_deref(),
            RunpodResourceKind::Endpoint => resources.endpoint_id.as_deref(),
        }
    }

    fn cleanup_resources_missing(kind: RunpodResourceKind) -> RunpodRuntimeResources {
        match kind {
            RunpodResourceKind::Endpoint => RunpodRuntimeResources {
                network_volume_id: Some("volume-1".into()),
                template_id: Some("template-1".into()),
                ..RunpodRuntimeResources::default()
            },
            RunpodResourceKind::Template | RunpodResourceKind::ProvisionerPod => {
                RunpodRuntimeResources {
                    network_volume_id: Some("volume-1".into()),
                    ..RunpodRuntimeResources::default()
                }
            }
            RunpodResourceKind::NetworkVolume => RunpodRuntimeResources::default(),
        }
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

    #[tokio::test(start_paused = true)]
    async fn cleanup_deadline_aborts_the_body_and_persists_failure() {
        let fakes = CleanupFakes::ready_runtime();
        fakes.provider.block_first_call();

        let (_, operation) = start_cleanup(&fakes).await.unwrap();
        fakes.provider.wait_until_first_call().await;
        tokio::time::advance(CLEANUP_DEADLINE).await;
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
    async fn cleanup_discovers_persists_and_deletes_missing_resource_ids() {
        let cases = [
            (
                RunpodResourceKind::Endpoint,
                RunpodCleanupStep::DeleteEndpoint,
            ),
            (
                RunpodResourceKind::Template,
                RunpodCleanupStep::DeleteTemplate,
            ),
            (
                RunpodResourceKind::ProvisionerPod,
                RunpodCleanupStep::TerminateProvisionerPod,
            ),
            (
                RunpodResourceKind::NetworkVolume,
                RunpodCleanupStep::DeleteNetworkVolume,
            ),
        ];

        for (kind, step) in cases {
            let fakes = CleanupFakes::ready_runtime_with_resources(cleanup_resources_missing(kind));
            fakes.provider.set_observation(
                kind,
                RunpodResourceObservation::Found("discovered-id".into()),
            );
            for absent in [
                RunpodResourceKind::Template,
                RunpodResourceKind::ProvisionerPod,
            ] {
                if absent != kind {
                    fakes
                        .provider
                        .set_observation(absent, RunpodResourceObservation::Absent);
                }
            }

            let (_, operation) = start_cleanup(&fakes).await.unwrap();
            fakes.events.wait_for_terminal_operation(operation.id).await;

            assert!(fakes
                .repository
                .resources_at_cleanup_step(step)
                .iter()
                .any(|resources| resource_id(resources, kind) == Some("discovered-id")));
            assert!(fakes
                .provider
                .deletion_attempts()
                .contains(&(kind, "discovered-id".into())));
            assert_eq!(
                fakes.repository.running_cleanup_steps(),
                match step {
                    RunpodCleanupStep::DeleteEndpoint => vec![
                        RunpodCleanupStep::DeleteEndpoint,
                        RunpodCleanupStep::DeleteEndpoint,
                        RunpodCleanupStep::DeleteTemplate,
                        RunpodCleanupStep::TerminateProvisionerPod,
                        RunpodCleanupStep::DeleteNetworkVolume,
                    ],
                    RunpodCleanupStep::DeleteTemplate => vec![
                        RunpodCleanupStep::DeleteEndpoint,
                        RunpodCleanupStep::DeleteTemplate,
                        RunpodCleanupStep::DeleteTemplate,
                        RunpodCleanupStep::TerminateProvisionerPod,
                        RunpodCleanupStep::DeleteNetworkVolume,
                    ],
                    RunpodCleanupStep::TerminateProvisionerPod => vec![
                        RunpodCleanupStep::DeleteEndpoint,
                        RunpodCleanupStep::DeleteTemplate,
                        RunpodCleanupStep::TerminateProvisionerPod,
                        RunpodCleanupStep::TerminateProvisionerPod,
                        RunpodCleanupStep::DeleteNetworkVolume,
                    ],
                    RunpodCleanupStep::DeleteNetworkVolume => vec![
                        RunpodCleanupStep::DeleteEndpoint,
                        RunpodCleanupStep::DeleteTemplate,
                        RunpodCleanupStep::TerminateProvisionerPod,
                        RunpodCleanupStep::DeleteNetworkVolume,
                        RunpodCleanupStep::DeleteNetworkVolume,
                    ],
                }
            );
            assert!(fakes.repository.runtime_was_removed());
        }
    }

    #[tokio::test]
    async fn cleanup_deletes_every_ambiguous_match_before_advancing() {
        let fakes = CleanupFakes::ready_runtime_with_resources(cleanup_resources_missing(
            RunpodResourceKind::Endpoint,
        ));
        fakes.provider.set_observation(
            RunpodResourceKind::Endpoint,
            RunpodResourceObservation::Ambiguous(vec!["endpoint-a".into(), "endpoint-b".into()]),
        );
        fakes.provider.set_observation(
            RunpodResourceKind::ProvisionerPod,
            RunpodResourceObservation::Absent,
        );

        let (_, operation) = start_cleanup(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(
            &fakes.provider.deletion_attempts()[..2],
            &[
                (RunpodResourceKind::Endpoint, "endpoint-a".into()),
                (RunpodResourceKind::Endpoint, "endpoint-b".into()),
            ]
        );
        assert!(fakes.repository.runtime_was_removed());
    }

    #[tokio::test]
    async fn cleanup_fails_when_any_ambiguous_delete_fails() {
        let fakes = CleanupFakes::ready_runtime_with_resources(cleanup_resources_missing(
            RunpodResourceKind::Endpoint,
        ));
        fakes.provider.set_observation(
            RunpodResourceKind::Endpoint,
            RunpodResourceObservation::Ambiguous(vec!["endpoint-a".into(), "endpoint-b".into()]),
        );
        fakes.provider.fail_once("delete_endpoint");

        let (_, started_operation) = start_cleanup(&fakes).await.unwrap();
        fakes
            .events
            .wait_for_terminal_operation(started_operation.id)
            .await;

        assert_eq!(
            fakes.provider.deletion_attempts(),
            vec![
                (RunpodResourceKind::Endpoint, "endpoint-a".into()),
                (RunpodResourceKind::Endpoint, "endpoint-b".into()),
            ]
        );
        let (runtime, operation) = fakes.repository.last_snapshot();
        assert_eq!(runtime.provision_operation_id, Uuid::from_u128(1));
        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
    }

    #[tokio::test]
    async fn cleanup_observation_failure_retains_generation_identity() {
        let fakes = CleanupFakes::ready_runtime_with_resources(cleanup_resources_missing(
            RunpodResourceKind::Endpoint,
        ));
        fakes.provider.fail_once_with(
            "observe_endpoint",
            RunpodRuntimeProviderError::ObserveUnavailable,
        );

        let (_, started_operation) = start_cleanup(&fakes).await.unwrap();
        fakes
            .events
            .wait_for_terminal_operation(started_operation.id)
            .await;

        let (runtime, operation) = fakes.repository.last_snapshot();
        assert_eq!(runtime.provision_operation_id, Uuid::from_u128(1));
        assert_eq!(runtime.resources.endpoint_id, None);
        assert_eq!(
            runtime.resources.network_volume_id.as_deref(),
            Some("volume-1")
        );
        assert_eq!(runtime.resources.template_id.as_deref(), Some("template-1"));
        assert_eq!(operation.state, RuntimeOperationState::Failed);
        assert_eq!(
            runpod_progress(operation.progress).cleanup_step(),
            Some(RunpodCleanupStep::DeleteEndpoint)
        );
    }

    #[tokio::test]
    async fn cleanup_skips_absent_resource_ids_but_still_records_each_step() {
        let fakes = CleanupFakes::failed_partial_runtime();
        fakes.provider.set_observation(
            RunpodResourceKind::Template,
            RunpodResourceObservation::Absent,
        );
        fakes.provider.set_observation(
            RunpodResourceKind::ProvisionerPod,
            RunpodResourceObservation::Absent,
        );

        let (_, operation) = start_cleanup(&fakes).await.unwrap();
        fakes.events.wait_for_terminal_operation(operation.id).await;

        assert_eq!(fakes.repository.running_cleanup_steps().len(), 4);
        assert_eq!(
            fakes.provider.calls(),
            vec![
                "observe_template",
                "observe_provisioner_pod",
                "delete_network_volume",
            ]
        );
    }

    #[tokio::test]
    async fn cleanup_without_runtime_is_explicit_not_provisioned() {
        let fakes = CleanupFakes::without_runtime();

        assert_eq!(
            start_cleanup(&fakes).await,
            Err(RuntimeError::NotProvisioned)
        );
        assert!(fakes.provider.calls().is_empty());
        assert!(fakes.events.events().is_empty());
    }

    #[tokio::test]
    async fn cleanup_without_runpod_credential_does_not_start_a_transition() {
        let fakes = CleanupFakes::ready_runtime_without_runpod_credential();

        assert_eq!(
            start_cleanup(&fakes).await,
            Err(RuntimeError::CredentialMissing)
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
        assert_eq!(provider.provision_operation_id, Uuid::from_u128(1));
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
}
