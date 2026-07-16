use time::OffsetDateTime;

use crate::application::{
    runtimes::{
        ports::RuntimePersistenceError, RuntimeError, RuntimeKind, RuntimeOperation,
        RuntimeProgress,
    },
    secrets::SecretKind,
    workspace::ports::WorkspaceRepositoryError,
};

use super::service::{mark_failed, runpod, runpod_mut, RunpodRuntimeService};
use super::{
    resource_name, ObserveEndpoint, ObserveNetworkVolume, ObserveProvisionerPod, ObserveTemplate,
    RunpodProgress, RunpodProvisionStep, RunpodResourceKind, RunpodResourceObservation,
};

#[derive(crate::diagnostics::DiagnosticDebug, thiserror::Error)]
enum RunpodRecoveryError {
    #[error("runtime recovery found corrupt persistence")]
    CorruptData,
    #[error("runtime recovery could not finish one valid operation")]
    Operation(RuntimeError),
}

impl RunpodRuntimeService {
    #[crate::diagnostics::diagnostic]
    pub async fn recover_interrupted(
        &self,
        operations: Vec<RuntimeOperation>,
    ) -> Result<(), RuntimeError> {
        for operation in operations {
            match self.recover_one(operation).await {
                Ok(()) => {}
                Err(RunpodRecoveryError::CorruptData) => {
                    return Err(RuntimeError::PersistenceUnavailable);
                }
                Err(RunpodRecoveryError::Operation(_)) => {}
            }
        }
        Ok(())
    }

    #[crate::diagnostics::diagnostic(restore = operation.trace_id)]
    async fn recover_one(&self, operation: RuntimeOperation) -> Result<(), RunpodRecoveryError> {
        let mut operation = operation;
        if operation.runtime_kind != RuntimeKind::Runpod {
            return Err(RunpodRecoveryError::Operation(
                RuntimeError::PersistenceUnavailable,
            ));
        }
        let mut workspace = match self.workspaces.get(&operation.workspace_id).await {
            Ok(Some(workspace)) => workspace,
            Ok(None) => {
                return Err(RunpodRecoveryError::Operation(RuntimeError::NotProvisioned));
            }
            Err(WorkspaceRepositoryError::CorruptData) => {
                return Err(RunpodRecoveryError::CorruptData);
            }
            Err(_) => {
                return Err(RunpodRecoveryError::Operation(
                    RuntimeError::PersistenceUnavailable,
                ));
            }
        };

        let RuntimeProgress::Runpod(progress) = operation.progress;
        if let RunpodProgress::Provision(step) = progress {
            match step {
                RunpodProvisionStep::CreateNetworkVolume
                    if runpod(&workspace)
                        .map_err(RunpodRecoveryError::Operation)?
                        .resources
                        .network_volume_id
                        .is_none() =>
                {
                    let runtime = runpod(&workspace).map_err(RunpodRecoveryError::Operation)?;
                    let command = ObserveNetworkVolume {
                        name: resource_name(
                            &workspace.id,
                            runtime.provision_operation_id,
                            RunpodResourceKind::NetworkVolume,
                        ),
                        datacenter_id: runtime.config.datacenter_id.clone(),
                        size_gb: runtime.config.volume_size_gb,
                    };
                    if let Ok(Some(runpod_key)) = self.secrets.get(SecretKind::RunpodApiKey).await {
                        match self
                            .provider
                            .observe_network_volume(&runpod_key, command)
                            .await
                        {
                            Ok(RunpodResourceObservation::Found(id)) => {
                                runpod_mut(&mut workspace)
                                    .map_err(RunpodRecoveryError::Operation)?
                                    .resources
                                    .network_volume_id = Some(id);
                            }
                            Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                for id in ids {
                                    let _ =
                                        self.provider.delete_network_volume(&runpod_key, &id).await;
                                }
                            }
                            Ok(RunpodResourceObservation::Absent) | Err(_) => {}
                        }
                    }
                }
                RunpodProvisionStep::StartProvisionerPod
                    if runpod(&workspace)
                        .map_err(RunpodRecoveryError::Operation)?
                        .resources
                        .provisioner_pod_id
                        .is_none() =>
                {
                    let runtime = runpod(&workspace).map_err(RunpodRecoveryError::Operation)?;
                    if let Some(network_volume_id) = runtime.resources.network_volume_id.clone() {
                        let command = ObserveProvisionerPod {
                            name: resource_name(
                                &workspace.id,
                                runtime.provision_operation_id,
                                RunpodResourceKind::ProvisionerPod,
                            ),
                            network_volume_id,
                        };
                        if let Ok(Some(runpod_key)) =
                            self.secrets.get(SecretKind::RunpodApiKey).await
                        {
                            match self
                                .provider
                                .observe_provisioner_pod(&runpod_key, command)
                                .await
                            {
                                Ok(RunpodResourceObservation::Found(id)) => {
                                    runpod_mut(&mut workspace)
                                        .map_err(RunpodRecoveryError::Operation)?
                                        .resources
                                        .provisioner_pod_id = Some(id);
                                }
                                Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                    for id in ids {
                                        let _ = self
                                            .provider
                                            .terminate_provisioner_pod(&runpod_key, &id)
                                            .await;
                                    }
                                }
                                Ok(RunpodResourceObservation::Absent) | Err(_) => {}
                            }
                        }
                    }
                }
                RunpodProvisionStep::CreateTemplate
                    if runpod(&workspace)
                        .map_err(RunpodRecoveryError::Operation)?
                        .resources
                        .template_id
                        .is_none() =>
                {
                    let runtime = runpod(&workspace).map_err(RunpodRecoveryError::Operation)?;
                    let command = ObserveTemplate {
                        name: resource_name(
                            &workspace.id,
                            runtime.provision_operation_id,
                            RunpodResourceKind::Template,
                        ),
                    };
                    if let Ok(Some(runpod_key)) = self.secrets.get(SecretKind::RunpodApiKey).await {
                        match self.provider.observe_template(&runpod_key, command).await {
                            Ok(RunpodResourceObservation::Found(id)) => {
                                runpod_mut(&mut workspace)
                                    .map_err(RunpodRecoveryError::Operation)?
                                    .resources
                                    .template_id = Some(id);
                            }
                            Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                for id in ids {
                                    let _ = self.provider.delete_template(&runpod_key, &id).await;
                                }
                            }
                            Ok(RunpodResourceObservation::Absent) | Err(_) => {}
                        }
                    }
                }
                RunpodProvisionStep::CreateEndpoint
                    if runpod(&workspace)
                        .map_err(RunpodRecoveryError::Operation)?
                        .resources
                        .endpoint_id
                        .is_none() =>
                {
                    let runtime = runpod(&workspace).map_err(RunpodRecoveryError::Operation)?;
                    if let (Some(network_volume_id), Some(template_id)) = (
                        runtime.resources.network_volume_id.clone(),
                        runtime.resources.template_id.clone(),
                    ) {
                        let command = ObserveEndpoint {
                            name: resource_name(
                                &workspace.id,
                                runtime.provision_operation_id,
                                RunpodResourceKind::Endpoint,
                            ),
                            gpu_id: runtime.config.gpu_id.clone(),
                            network_volume_id,
                            template_id,
                        };
                        if let Ok(Some(runpod_key)) =
                            self.secrets.get(SecretKind::RunpodApiKey).await
                        {
                            match self.provider.observe_endpoint(&runpod_key, command).await {
                                Ok(RunpodResourceObservation::Found(id)) => {
                                    runpod_mut(&mut workspace)
                                        .map_err(RunpodRecoveryError::Operation)?
                                        .resources
                                        .endpoint_id = Some(id);
                                }
                                Ok(RunpodResourceObservation::Ambiguous(ids)) => {
                                    for id in ids {
                                        let _ =
                                            self.provider.delete_endpoint(&runpod_key, &id).await;
                                    }
                                }
                                Ok(RunpodResourceObservation::Absent) | Err(_) => {}
                            }
                        }
                    }
                }
                RunpodProvisionStep::PollProvisioner
                | RunpodProvisionStep::TerminateProvisionerPod
                | RunpodProvisionStep::CreateNetworkVolume
                | RunpodProvisionStep::StartProvisionerPod
                | RunpodProvisionStep::CreateTemplate
                | RunpodProvisionStep::CreateEndpoint => {}
            }
        }

        mark_failed(&mut workspace).map_err(RunpodRecoveryError::Operation)?;
        operation
            .fail(OffsetDateTime::now_utc())
            .map_err(Into::<RuntimeError>::into)
            .map_err(RunpodRecoveryError::Operation)?;
        match self.transitions.save(&workspace, &operation).await {
            Ok(()) => Ok(()),
            Err(RuntimePersistenceError::CorruptData) => Err(RunpodRecoveryError::CorruptData),
            Err(_) => Err(RunpodRecoveryError::Operation(
                RuntimeError::PersistenceUnavailable,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::application::runtimes::runpod::{
        test_support::{runpod_progress, RecoveryFakes},
        RunpodCleanupStep, RunpodProgress, RunpodProvisionStep, RunpodResourceKind,
        RunpodResourceObservation, RunpodRuntimeProviderError, RunpodRuntimeResources,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            ports::RuntimePersistenceError, RuntimeError, RuntimeOperationState, RuntimeProgress,
            RuntimeState,
        },
        secrets::SecretStoreError,
        workspace::ports::WorkspaceRepositoryError,
    };

    #[crate::diagnostics::diagnostic(root)]
    async fn fail_interrupted(fakes: &RecoveryFakes) -> Result<(), RuntimeError> {
        fakes
            .service()
            .recover_interrupted(fakes.running_operations())
            .await
    }

    fn recovery_resources(step: RunpodProvisionStep) -> RunpodRuntimeResources {
        let mut resources = RunpodRuntimeResources::default();
        match step {
            RunpodProvisionStep::CreateNetworkVolume => {}
            RunpodProvisionStep::StartProvisionerPod => {
                resources.network_volume_id = Some("volume-1".into());
            }
            RunpodProvisionStep::PollProvisioner | RunpodProvisionStep::TerminateProvisionerPod => {
                resources.network_volume_id = Some("volume-1".into());
                resources.provisioner_pod_id = Some("pod-1".into());
            }
            RunpodProvisionStep::CreateTemplate => {
                resources.network_volume_id = Some("volume-1".into());
            }
            RunpodProvisionStep::CreateEndpoint => {
                resources.network_volume_id = Some("volume-1".into());
                resources.template_id = Some("template-1".into());
            }
        }
        resources
    }

    fn set_resource_id(resources: &mut RunpodRuntimeResources, kind: RunpodResourceKind, id: &str) {
        match kind {
            RunpodResourceKind::NetworkVolume => resources.network_volume_id = Some(id.into()),
            RunpodResourceKind::ProvisionerPod => resources.provisioner_pod_id = Some(id.into()),
            RunpodResourceKind::Template => resources.template_id = Some(id.into()),
            RunpodResourceKind::Endpoint => resources.endpoint_id = Some(id.into()),
        }
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
        assert_eq!(fakes.provider.calls(), vec!["observe_network_volume"]);
    }

    #[tokio::test]
    async fn corrupt_workspace_data_stops_recovery() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();
        fakes.fail_workspace_read_once("workspace-1", WorkspaceRepositoryError::CorruptData);

        assert_eq!(
            fail_interrupted(&fakes).await,
            Err(RuntimeError::PersistenceUnavailable)
        );
        assert!(fakes.repository.saved_states().is_empty());
        assert!(fakes.provider.calls().is_empty());
    }

    #[tokio::test]
    async fn corrupt_transition_data_stops_recovery() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();
        fakes.set_runtime_resources(
            "workspace-1",
            recovery_resources(RunpodProvisionStep::CreateEndpoint),
        );
        fakes.provider.set_observation(
            RunpodResourceKind::Endpoint,
            RunpodResourceObservation::Absent,
        );
        fakes.fail_next_transition_with(RuntimePersistenceError::CorruptData);

        assert_eq!(
            fail_interrupted(&fakes).await,
            Err(RuntimeError::PersistenceUnavailable)
        );
        assert!(fakes.repository.saved_states().is_empty());
        assert_eq!(fakes.provider.calls(), vec!["observe_endpoint"]);
    }

    #[tokio::test]
    async fn one_valid_recovery_failure_does_not_stop_later_operations() {
        let read_failure = RecoveryFakes::with_running_provision_and_cleanup();
        read_failure.fail_workspace_read_once("workspace-1", WorkspaceRepositoryError::Unavailable);
        read_failure.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Absent,
        );

        fail_interrupted(&read_failure).await.unwrap();

        assert_eq!(read_failure.repository.saved_states().len(), 2);
        assert_eq!(
            read_failure.provider.calls(),
            vec!["observe_network_volume"]
        );

        let save_failure = RecoveryFakes::with_running_provision_and_cleanup();
        save_failure.set_runtime_resources(
            "workspace-1",
            recovery_resources(RunpodProvisionStep::CreateEndpoint),
        );
        save_failure.provider.set_observation(
            RunpodResourceKind::Endpoint,
            RunpodResourceObservation::Absent,
        );
        save_failure.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Absent,
        );
        save_failure.fail_next_transition_with(RuntimePersistenceError::Unavailable);

        fail_interrupted(&save_failure).await.unwrap();

        assert_eq!(save_failure.repository.saved_states().len(), 2);
        assert_eq!(
            save_failure.provider.calls(),
            vec!["observe_endpoint", "observe_network_volume"]
        );
    }

    #[tokio::test]
    async fn provision_recovery_observes_only_the_resource_implied_by_progress() {
        let cases = [
            (
                RunpodProvisionStep::CreateNetworkVolume,
                Some((RunpodResourceKind::NetworkVolume, "observe_network_volume")),
            ),
            (
                RunpodProvisionStep::StartProvisionerPod,
                Some((
                    RunpodResourceKind::ProvisionerPod,
                    "observe_provisioner_pod",
                )),
            ),
            (RunpodProvisionStep::PollProvisioner, None),
            (RunpodProvisionStep::TerminateProvisionerPod, None),
            (
                RunpodProvisionStep::CreateTemplate,
                Some((RunpodResourceKind::Template, "observe_template")),
            ),
            (
                RunpodProvisionStep::CreateEndpoint,
                Some((RunpodResourceKind::Endpoint, "observe_endpoint")),
            ),
        ];

        for (step, expected) in cases {
            let fakes = RecoveryFakes::with_running_provision_and_cleanup();
            fakes.set_runtime_resources("workspace-1", recovery_resources(step));
            let mut operation = fakes.running_operations().remove(0);
            operation.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(step));
            if let Some((kind, _)) = expected {
                fakes
                    .provider
                    .set_observation(kind, RunpodResourceObservation::Absent);
            }

            fakes
                .service()
                .recover_interrupted(vec![operation])
                .await
                .unwrap();

            assert_eq!(
                fakes.provider.calls(),
                expected.map_or_else(Vec::new, |(_, method)| vec![method])
            );
            assert_eq!(
                fakes.runpod_secret_read_count(),
                usize::from(expected.is_some())
            );

            if let Some((kind, _)) = expected {
                let durable = RecoveryFakes::with_running_provision_and_cleanup();
                let mut resources = recovery_resources(step);
                set_resource_id(&mut resources, kind, "durable-id");
                durable.set_runtime_resources("workspace-1", resources);
                let mut operation = durable.running_operations().remove(0);
                operation.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(step));

                durable
                    .service()
                    .recover_interrupted(vec![operation])
                    .await
                    .unwrap();

                assert!(durable.provider.calls().is_empty());
                assert_eq!(durable.runpod_secret_read_count(), 0);
            }
        }
    }

    #[tokio::test]
    async fn recovery_without_credential_marks_failed_and_continues() {
        let missing = RecoveryFakes::with_running_provision_and_cleanup();
        missing.set_runtime_resources(
            "workspace-1",
            recovery_resources(RunpodProvisionStep::CreateEndpoint),
        );
        missing.remove_runpod_credential();

        fail_interrupted(&missing).await.unwrap();

        assert_eq!(missing.repository.saved_states().len(), 3);
        assert_eq!(missing.runpod_secret_read_count(), 2);
        assert!(missing.provider.calls().is_empty());

        let unavailable = RecoveryFakes::with_running_provision_and_cleanup();
        unavailable.set_runtime_resources(
            "workspace-1",
            recovery_resources(RunpodProvisionStep::CreateEndpoint),
        );
        unavailable.fail_runpod_secret_read_once(SecretStoreError::Unavailable);
        unavailable.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Absent,
        );

        fail_interrupted(&unavailable).await.unwrap();

        assert_eq!(unavailable.repository.saved_states().len(), 3);
        assert_eq!(unavailable.runpod_secret_read_count(), 2);
        assert_eq!(unavailable.provider.calls(), vec!["observe_network_volume"]);
    }

    #[tokio::test]
    async fn recovery_observation_failure_marks_failed_and_continues() {
        let fakes = RecoveryFakes::with_running_provision_and_cleanup();
        fakes.set_runtime_resources(
            "workspace-1",
            recovery_resources(RunpodProvisionStep::CreateEndpoint),
        );
        fakes.provider.fail_once_with(
            "observe_endpoint",
            RunpodRuntimeProviderError::ObserveUnavailable,
        );
        fakes.provider.set_observation(
            RunpodResourceKind::NetworkVolume,
            RunpodResourceObservation::Absent,
        );

        fail_interrupted(&fakes).await.unwrap();

        assert_eq!(fakes.repository.saved_states().len(), 3);
        assert_eq!(
            fakes.provider.calls(),
            vec!["observe_endpoint", "observe_network_volume"]
        );
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
