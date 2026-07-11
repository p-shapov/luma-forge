use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    catalog::WorkflowDefinition,
    lifecycle::{
        ports::LifecycleOperationRepository, progress::runpod::RunpodProvisionStep,
        LifecycleOperation,
    },
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

pub struct RunpodRuntimeService<'a> {
    pub workspaces: &'a dyn WorkspaceRepository,
    pub workflows: &'a dyn WorkflowCatalog,
    pub runtimes: &'a dyn RunpodRuntimeRepository,
    pub runtime_catalog: &'a dyn RunpodRuntimeCatalog,
    pub lifecycle: &'a dyn LifecycleOperationRepository,
    pub secrets: &'a dyn SecretStore,
    pub provider: &'a dyn RunpodRuntimeProvider,
}

impl RunpodRuntimeService<'_> {
    pub async fn provision(
        &self,
        command: ProvisionRunpodRuntime,
    ) -> Result<RunpodRuntime, RunpodRuntimeError> {
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

        let mut runtime = RunpodRuntime::new_provisioning(
            command.workspace_id.clone(),
            RunpodRuntimeConfig {
                datacenter_id: command.datacenter_id.clone(),
                gpu_id: command.gpu_id.clone(),
                volume_size_gb: command.volume_size_gb,
            },
        );
        let mut operation = LifecycleOperation::runpod_provision(
            Uuid::new_v4(),
            &command.workspace_id,
            Uuid::new_v4(),
            RunpodProvisionStep::CreateNetworkVolume,
            OffsetDateTime::now_utc(),
        );
        self.runtimes.save_transition(&runtime, &operation).await?;

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
                return Err(RunpodRuntimeError::from(error));
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
                    provisioner_image_ref: definition.provisioner_contract.image_ref,
                    required_model_assets: workflow.model_assets,
                    hugging_face_api_key,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(RunpodRuntimeError::from(error));
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
                return Err(RunpodRuntimeError::from(error));
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
                return Err(RunpodRuntimeError::from(error));
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
                    image_ref: definition.endpoint_contract.image_ref,
                },
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                self.fail_transition(&mut runtime, &mut operation).await?;
                return Err(RunpodRuntimeError::from(error));
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
                return Err(RunpodRuntimeError::from(error));
            }
        };
        runtime.resources.endpoint_id = Some(endpoint_id);

        runtime.mark_ready()?;
        operation.succeed(OffsetDateTime::now_utc())?;
        self.runtimes.save_transition(&runtime, &operation).await?;

        Ok(runtime)
    }

    async fn set_provision_step(
        &self,
        runtime: &RunpodRuntime,
        operation: &mut LifecycleOperation,
        step: RunpodProvisionStep,
    ) -> Result<(), RunpodRuntimeError> {
        operation.set_provision_step(step, OffsetDateTime::now_utc())?;
        self.runtimes
            .save_transition(runtime, operation)
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
        self.runtimes
            .save_transition(runtime, operation)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::application::lifecycle::{
        progress::runpod::RunpodProvisionStep, LifecycleOperationState,
    };

    use super::ProvisionRunpodRuntime;
    use crate::application::runtimes::runpod::{
        test_support::ProvisionFakes, RunpodRuntimeError, RunpodRuntimeResources,
        RunpodRuntimeState,
    };

    #[tokio::test]
    async fn provision_persists_each_current_step_before_the_provider_call() {
        let fakes = ProvisionFakes::ready();

        let runtime = fakes
            .service()
            .provision(ProvisionRunpodRuntime {
                workspace_id: "workspace-1".into(),
                datacenter_id: "dc-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb: 19,
            })
            .await
            .unwrap();

        assert_eq!(runtime.state, RunpodRuntimeState::Ready);
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

            let error = fakes
                .service()
                .provision(ProvisionRunpodRuntime {
                    workspace_id: "workspace-1".into(),
                    datacenter_id: "dc-1".into(),
                    gpu_id: "gpu-1".into(),
                    volume_size_gb: 19,
                })
                .await
                .unwrap_err();

            assert_eq!(error, RunpodRuntimeError::ProviderUnavailable, "{method}");
            assert_eq!(fakes.provider.calls(), methods[..=index], "{method}");
            let (runtime, operation) = fakes.repository.last_snapshot();
            assert_eq!(runtime.state, RunpodRuntimeState::Failed, "{method}");
            assert_eq!(runtime.resources, resources, "{method}");
            assert_eq!(operation.state, LifecycleOperationState::Failed, "{method}");
            assert_eq!(operation.progress.provision_step(), Some(step), "{method}");
        }
    }
}
