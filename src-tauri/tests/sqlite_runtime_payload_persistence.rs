use luma_forge_lib::{
    adapters::sqlite::{
        SqliteRuntimeOperationRepository, SqliteRuntimeTransitionRepository,
        SqliteWorkspaceRepository,
    },
    application::{
        runtimes::{
            ports::{
                RuntimeOperationRepository, RuntimeOperationRepositoryError,
                RuntimePersistenceError, RuntimeTransitionRepository,
            },
            runpod::{
                RunpodCleanupStep, RunpodProgress, RunpodProvisionStep, RunpodRuntime,
                RunpodRuntimeConfig,
            },
            CatalogRef, Runtime, RuntimeKind, RuntimeOperation, RuntimeOperationKind,
            RuntimeProgress, RuntimeProvider, RuntimeState,
        },
        workspace::{
            ports::{WorkspaceRepository, WorkspaceRepositoryError},
            Workspace,
        },
    },
    infra::sqlite::{
        database::SqliteInfraDatabase,
        entities::{runtime_operations, workspace_runtimes},
    },
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, EntityTrait, IntoActiveModel};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

struct Fixture {
    database: SqliteInfraDatabase,
    workspaces: SqliteWorkspaceRepository,
    operations: SqliteRuntimeOperationRepository,
    transitions: SqliteRuntimeTransitionRepository,
}

impl Fixture {
    async fn new() -> Self {
        let path = std::env::temp_dir().join(format!("luma-forge-{}.sqlite", Uuid::new_v4()));
        let database = SqliteInfraDatabase::connect(path).await.unwrap();
        let connection = database.connection().clone();
        Self {
            database,
            workspaces: SqliteWorkspaceRepository::new(connection.clone()),
            operations: SqliteRuntimeOperationRepository::new(connection.clone()),
            transitions: SqliteRuntimeTransitionRepository::new(connection),
        }
    }

    async fn with_ready_runtime() -> Self {
        let fixture = Self::new().await;
        let mut workspace = fixture.workspace("workspace-1");
        fixture.workspaces.create(workspace.clone()).await.unwrap();
        let operation_id = Uuid::new_v4();
        workspace.runtime = Some(runpod_runtime(operation_id, RuntimeState::Ready, 100));
        let mut operation =
            running_operation(operation_id, &workspace.id, RuntimeOperationKind::Provision);
        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await
            .unwrap();
        fixture
    }

    fn workspace(&self, id: &str) -> Workspace {
        Workspace {
            id: id.into(),
            workflow: CatalogRef::new("workflow-1", "1"),
            created_at: OffsetDateTime::UNIX_EPOCH,
            runtime: None,
        }
    }
}

fn runpod_runtime(
    provision_operation_id: Uuid,
    state: RuntimeState,
    volume_size_gb: u64,
) -> Runtime {
    Runtime {
        state,
        provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(
            provision_operation_id,
            RunpodRuntimeConfig {
                datacenter_id: "EU-RO-1".into(),
                gpu_id: "gpu-1".into(),
                volume_size_gb,
            },
        )),
    }
}

fn running_operation(id: Uuid, workspace_id: &str, kind: RuntimeOperationKind) -> RuntimeOperation {
    let progress = match kind {
        RuntimeOperationKind::Provision => {
            RunpodProgress::Provision(RunpodProvisionStep::CreateNetworkVolume)
        }
        RuntimeOperationKind::Cleanup => RunpodProgress::Cleanup(RunpodCleanupStep::DeleteEndpoint),
    };
    RuntimeOperation::running(
        id,
        workspace_id,
        RuntimeKind::Runpod,
        kind,
        RuntimeProgress::Runpod(progress),
        OffsetDateTime::UNIX_EPOCH,
    )
}

#[tokio::test]
async fn workspace_get_and_page_round_trip_inline_provider_payload() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    let provision_operation_id = Uuid::from_u128(1);
    workspace.runtime = Some(runpod_runtime(
        provision_operation_id,
        RuntimeState::Provisioning,
        100,
    ));
    let runpod = workspace
        .runtime
        .as_mut()
        .unwrap()
        .provider
        .as_runpod_mut()
        .unwrap();
    runpod.resources.network_volume_id = Some("network-volume-1".into());
    runpod.resources.template_id = Some("template-1".into());
    let operation = running_operation(
        provision_operation_id,
        &workspace.id,
        RuntimeOperationKind::Provision,
    );

    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    let stored = fixture.workspaces.get(&workspace.id).await.unwrap();
    assert_eq!(
        stored
            .as_ref()
            .unwrap()
            .runtime
            .as_ref()
            .unwrap()
            .provider
            .as_runpod()
            .unwrap()
            .provision_operation_id,
        provision_operation_id
    );
    assert_eq!(stored, Some(workspace.clone()));
    assert_eq!(
        fixture.workspaces.page(0, 10).await.unwrap(),
        (vec![workspace.clone()], 1)
    );

    let anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(anchor.runtime_kind, "runpod");
    assert_eq!(anchor.state, "provisioning");
    assert_eq!(
        serde_json::from_str::<RuntimeProvider>(&anchor.provider_payload).unwrap(),
        workspace.runtime.unwrap().provider
    );
}

#[tokio::test]
async fn workspace_page_is_stable_and_reports_total() {
    let fixture = Fixture::new().await;
    let mut expected = fixture.workspace("workspace-2");

    for id in ["workspace-1", "workspace-2", "workspace-3"] {
        let mut workspace = fixture.workspace(id);
        fixture.workspaces.create(workspace.clone()).await.unwrap();
        if id != "workspace-1" {
            let operation_id = Uuid::new_v4();
            workspace.runtime = Some(runpod_runtime(
                operation_id,
                RuntimeState::Provisioning,
                100,
            ));
            let operation = running_operation(operation_id, id, RuntimeOperationKind::Provision);
            fixture
                .transitions
                .save_transition(&workspace, &operation)
                .await
                .unwrap();
            if id == "workspace-2" {
                expected = workspace;
            }
        }
    }

    let (items, total) = fixture.workspaces.page(1, 1).await.unwrap();

    assert_eq!(total, 3);
    assert_eq!(items, vec![expected]);
}

#[tokio::test]
async fn operation_reads_keep_progress_filtering_ordering_totals_and_recovery() {
    let fixture = Fixture::with_ready_runtime().await;

    let mut cleanup_workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    cleanup_workspace.runtime.as_mut().unwrap().state = RuntimeState::CleaningUp;
    let mut cleanup =
        running_operation(Uuid::new_v4(), "workspace-1", RuntimeOperationKind::Cleanup);
    cleanup.created_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(1);
    cleanup.updated_at = cleanup.created_at;
    fixture
        .transitions
        .save_transition(&cleanup_workspace, &cleanup)
        .await
        .unwrap();

    let mut provision_workspace = fixture.workspace("workspace-2");
    fixture
        .workspaces
        .create(provision_workspace.clone())
        .await
        .unwrap();
    let provision_id = Uuid::new_v4();
    provision_workspace.runtime = Some(runpod_runtime(
        provision_id,
        RuntimeState::Provisioning,
        120,
    ));
    let mut provision =
        running_operation(provision_id, "workspace-2", RuntimeOperationKind::Provision);
    provision.created_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(2);
    provision.updated_at = provision.created_at;
    fixture
        .transitions
        .save_transition(&provision_workspace, &provision)
        .await
        .unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 2).await.unwrap(),
        (vec![provision.clone(), cleanup.clone()], 3)
    );
    assert_eq!(
        fixture.operations.page(None, 1, 1).await.unwrap(),
        (vec![cleanup.clone()], 3)
    );
    let (workspace_operations, workspace_total) = fixture
        .operations
        .page(Some("workspace-1"), 0, 10)
        .await
        .unwrap();
    assert_eq!(workspace_total, 2);
    assert_eq!(workspace_operations.first(), Some(&cleanup));

    let mut running = fixture.operations.running().await.unwrap();
    running.sort_by_key(|operation| operation.created_at);
    assert_eq!(running, vec![cleanup, provision]);
    assert_eq!(
        fixture.operations.has_running("workspace-1").await,
        Ok(true)
    );
    assert_eq!(
        fixture.operations.has_running("workspace-2").await,
        Ok(true)
    );
}

#[tokio::test]
async fn database_failure_rolls_back_anchor_and_operation() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    let operation_id = Uuid::new_v4();
    workspace.runtime = Some(runpod_runtime(
        operation_id,
        RuntimeState::Provisioning,
        100,
    ));
    let operation = running_operation(operation_id, &workspace.id, RuntimeOperationKind::Provision);
    fixture
        .database
        .connection()
        .execute_unprepared(
            "CREATE TRIGGER fail_workspace_runtime_insert
             BEFORE INSERT ON workspace_runtimes
             BEGIN
                 SELECT RAISE(ABORT, 'forced persistence failure');
             END",
        )
        .await
        .unwrap();

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await,
        Err(RuntimePersistenceError::Unavailable),
    );
    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    assert!(
        runtime_operations::Entity::find_by_id(operation.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn progress_family_mismatch_is_rejected_on_write_and_read() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(
        Uuid::new_v4(),
        RuntimeState::CleaningUp,
        100,
    ));
    let mut invalid =
        running_operation(Uuid::new_v4(), &workspace.id, RuntimeOperationKind::Cleanup);
    invalid.progress = RuntimeProgress::Runpod(RunpodProgress::Provision(
        RunpodProvisionStep::CreateNetworkVolume,
    ));

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &invalid)
            .await,
        Err(RuntimePersistenceError::CorruptData),
    );
    assert!(
        runtime_operations::Entity::find_by_id(invalid.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());

    let valid_id = Uuid::new_v4();
    workspace.runtime = Some(runpod_runtime(valid_id, RuntimeState::Provisioning, 100));
    let valid = running_operation(valid_id, &workspace.id, RuntimeOperationKind::Provision);
    fixture
        .transitions
        .save_transition(&workspace, &valid)
        .await
        .unwrap();
    let mut stored = runtime_operations::Entity::find_by_id(valid.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    stored.operation_kind = Set("cleanup".into());
    stored.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 10).await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.operations.running().await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn provision_admission_rejection_rolls_back_operation_and_anchor_changes() {
    let fixture = Fixture::with_ready_runtime().await;
    let original = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    let operation_id = Uuid::new_v4();
    let mut attempted = original.clone();
    attempted.runtime = Some(runpod_runtime(
        operation_id,
        RuntimeState::Provisioning,
        200,
    ));
    let operation = running_operation(operation_id, "workspace-1", RuntimeOperationKind::Provision);

    assert_eq!(
        fixture
            .transitions
            .save_transition(&attempted, &operation)
            .await,
        Err(RuntimePersistenceError::OperationAlreadyRunning)
    );
    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Ok(Some(original))
    );
    assert!(
        runtime_operations::Entity::find_by_id(operation.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn cleanup_admission_rejects_an_anchor_already_in_transition() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    workspace.runtime.as_mut().unwrap().state = RuntimeState::CleaningUp;

    let mut anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    anchor.state = Set("cleaning_up".into());
    anchor.update(fixture.database.connection()).await.unwrap();

    let operation = running_operation(Uuid::new_v4(), "workspace-1", RuntimeOperationKind::Cleanup);

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await,
        Err(RuntimePersistenceError::OperationAlreadyRunning),
    );
    assert!(
        runtime_operations::Entity::find_by_id(operation.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Ok(Some(workspace))
    );
}

#[tokio::test]
async fn successful_cleanup_removes_anchor_and_keeps_terminal_progress() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    workspace.runtime = None;
    let mut operation =
        running_operation(Uuid::new_v4(), "workspace-1", RuntimeOperationKind::Cleanup);
    operation.progress = RuntimeProgress::Runpod(RunpodProgress::Cleanup(
        RunpodCleanupStep::DeleteNetworkVolume,
    ));
    operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();

    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    assert!(workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Ok(Some(workspace))
    );
    let stored = fixture
        .operations
        .page(Some("workspace-1"), 0, 10)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|stored| stored.id == operation.id)
        .unwrap();
    assert_eq!(stored, operation);
}

#[tokio::test]
async fn malformed_provider_payload_fails_workspace_get_and_page() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    anchor.provider_payload = Set("{".into());
    anchor.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.workspaces.page(0, 10).await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn provider_payload_tag_disagreement_is_corrupt() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    anchor.runtime_kind = Set("other".into());
    anchor.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn malformed_progress_payload_fails_operation_page_and_running() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    let operation_id = Uuid::new_v4();
    workspace.runtime = Some(runpod_runtime(
        operation_id,
        RuntimeState::Provisioning,
        100,
    ));
    let operation = running_operation(operation_id, &workspace.id, RuntimeOperationKind::Provision);
    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    let mut stored = runtime_operations::Entity::find_by_id(operation.id.to_string())
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap()
        .into_active_model();
    stored.progress_payload = Set("{".into());
    stored.update(fixture.database.connection()).await.unwrap();

    assert_eq!(
        fixture.operations.page(None, 0, 10).await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
    assert_eq!(
        fixture.operations.running().await,
        Err(RuntimeOperationRepositoryError::CorruptData)
    );
}

#[tokio::test]
async fn workspace_delete_distinguishes_eligible_and_missing_rows() {
    let fixture = Fixture::new().await;
    fixture
        .workspaces
        .create(fixture.workspace("workspace-1"))
        .await
        .unwrap();

    assert_eq!(fixture.workspaces.delete("workspace-1").await, Ok(true));
    assert_eq!(fixture.workspaces.delete("workspace-1").await, Ok(false));
}
