use std::{future::Future, sync::Arc, time::Duration};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::application::{
    events::ApplicationEventSink,
    runtimes::{
        ports::{RuntimeOperationRepository, RuntimePersistenceError, RuntimeTransitionRepository},
        Runtime, RuntimeContractRequirements, RuntimeError, RuntimeKind, RuntimeOperation,
        RuntimeOperationKind, RuntimeOperationState, RuntimeProgress, RuntimeProvider,
        RuntimeState, RuntimeTransitionContext, WorkflowDefinition,
    },
    secrets::{SecretKind, SecretStore},
    workspace::{
        ports::{WorkflowCatalog, WorkspaceRepository, WorkspaceRepositoryError},
        Workspace,
    },
};

use super::{
    resource_name, CreateEndpoint, CreateNetworkVolume, CreateTemplate, ObserveEndpoint,
    ObserveNetworkVolume, ObserveProvisionerPod, ObserveTemplate, RunpodCleanupStep,
    RunpodContractRequirements, RunpodPlacement, RunpodProgress, RunpodProvisionStep,
    RunpodResourceKind, RunpodResourceObservation, RunpodRuntime, RunpodRuntimeCatalog,
    RunpodRuntimeConfig, RunpodRuntimeDefinition, RunpodRuntimeProvider,
    RunpodRuntimeProviderError, StartProvisionerPod, RUNPOD_NETWORK_VOLUME_MAX_SIZE_GB,
};

const PROVISION_DEADLINE: Duration = Duration::from_secs(2 * 60 * 60);
const CLEANUP_DEADLINE: Duration = Duration::from_secs(5 * 60);

#[derive(crate::diagnostics::DiagnosticDebug, Clone, Copy)]
enum LifecycleTermination {
    BodyError,
    Deadline,
    Panic,
    Cancelled,
}

#[derive(crate::diagnostics::DiagnosticDebug, thiserror::Error)]
enum RunpodRecoveryError {
    #[error("runtime recovery found corrupt persistence")]
    CorruptData,
    #[error("runtime recovery could not finish one valid operation")]
    Operation(RuntimeError),
}

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

fn mark_failed(workspace: &mut Workspace) -> Result<(), RuntimeError> {
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

fn runpod(workspace: &Workspace) -> Result<&RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_ref()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod()
        .ok_or(RuntimeError::InvalidTransition)
}

fn runpod_mut(workspace: &mut Workspace) -> Result<&mut RunpodRuntime, RuntimeError> {
    let runtime = workspace
        .runtime
        .as_mut()
        .ok_or(RuntimeError::NotProvisioned)?;
    runtime
        .provider
        .as_runpod_mut()
        .ok_or(RuntimeError::InvalidTransition)
}

fn runpod_requirements(
    requirements: &[RuntimeContractRequirements],
) -> Result<&RunpodContractRequirements, RuntimeError> {
    requirements
        .iter()
        .find_map(RuntimeContractRequirements::as_runpod)
        .ok_or(RuntimeError::CatalogUnavailable)
}

#[derive(Clone)]
pub struct RunpodRuntimeService {
    workspaces: Arc<dyn WorkspaceRepository>,
    operations: Arc<dyn RuntimeOperationRepository>,
    workflows: Arc<dyn WorkflowCatalog>,
    runtime_catalog: Arc<dyn RunpodRuntimeCatalog>,
    secrets: Arc<dyn SecretStore>,
    provider: Arc<dyn RunpodRuntimeProvider>,
    transitions: RuntimeTransitionContext,
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

    #[crate::diagnostics::diagnostic(show_output, show_error)]
    pub async fn placement(&self) -> Result<RunpodPlacement, RuntimeError> {
        let key = self
            .secrets
            .get(SecretKind::RunpodApiKey)
            .await?
            .ok_or(RuntimeError::CredentialMissing)?;
        self.provider.placement(&key).await.map_err(Into::into)
    }

    #[crate::diagnostics::diagnostic]
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

    #[crate::diagnostics::diagnostic(detached, show_error)]
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

    fn spawn_supervised<F>(&self, operation_id: Uuid, deadline: Duration, body: F)
    where
        F: Future<Output = Result<(), RuntimeError>> + Send + 'static,
    {
        let body = tokio::spawn(body);
        let service = self.clone();
        tokio::spawn(async move {
            let _ = service.supervise(operation_id, deadline, body).await;
        });
    }

    #[crate::diagnostics::diagnostic(detached, show_error)]
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

    #[crate::diagnostics::diagnostic(restore = operation.trace_id, show_error)]
    async fn terminalize_supervised_operation(
        &self,
        mut workspace: Workspace,
        mut operation: RuntimeOperation,
        #[diagnostic(show)] _termination: LifecycleTermination,
    ) -> Result<(), RuntimeError> {
        self.fail_transition(&mut workspace, &mut operation).await
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
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::application::runtimes::runpod::{
        test_support::{provision_command, CleanupFakes, ProvisionFakes, RecoveryFakes},
        RunpodPlacement, RunpodPlacementDatacenter, RunpodPlacementGpu, RunpodResourceKind,
        RunpodResourceObservation, RunpodRuntimeProviderError, RunpodRuntimeResources,
    };
    use crate::application::{
        events::ApplicationEvent,
        runtimes::{
            ports::{RuntimePersistenceError, RuntimeTransitionRepository},
            runpod::{
                RunpodCleanupStep, RunpodContractRequirements, RunpodProgress, RunpodProvisionStep,
            },
            CatalogRef, Runtime, RuntimeContractRequirements, RuntimeError, RuntimeKind,
            RuntimeOperation, RuntimeOperationState, RuntimeProgress, RuntimeProvider,
            RuntimeState,
        },
        secrets::SecretStoreError,
        workspace::{ports::WorkspaceRepositoryError, Workspace},
    };

    use super::{CLEANUP_DEADLINE, PROVISION_DEADLINE};

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

    #[crate::diagnostics::diagnostic(root)]
    async fn start_cleanup(
        fakes: &CleanupFakes,
    ) -> Result<(Workspace, RuntimeOperation), RuntimeError> {
        fakes
            .service()
            .start_cleanup(fakes.workspace_snapshot())
            .await
    }

    #[crate::diagnostics::diagnostic(root)]
    async fn fail_interrupted(fakes: &RecoveryFakes) -> Result<(), RuntimeError> {
        fakes
            .service()
            .recover_interrupted(fakes.running_operations())
            .await
    }

    fn runpod_progress(progress: RuntimeProgress) -> RunpodProgress {
        let RuntimeProgress::Runpod(progress) = progress;
        progress
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

    async fn wait_for_deletion_attempts(fakes: &ProvisionFakes, count: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while fakes.provider.deletion_attempts().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    async fn yield_until(mut condition: impl FnMut() -> bool) {
        for _ in 0..100 {
            if condition() {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

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
