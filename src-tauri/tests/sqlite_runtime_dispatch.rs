use luma_forge_lib::{
    adapters::sqlite::{
        SqliteRuntimeOperationRepository, SqliteRuntimeTransitionRepository,
        SqliteWorkspaceRepository,
    },
    application::{
        runtimes::{
            ports::{
                RuntimeOperationRepository, RuntimePersistenceError, RuntimeTransitionRepository,
            },
            runpod::{RunpodProgress, RunpodProvisionStep, RunpodRuntime, RunpodRuntimeConfig},
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
        entities::{
            runpod_runtime_operation_progress, runpod_workspace_runtimes, runtime_operations,
            workspace_runtimes,
        },
    },
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use time::OffsetDateTime;
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
        workspace.runtime = Some(runpod_runtime(RuntimeState::Ready, 100));
        let mut operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);
        operation.succeed(OffsetDateTime::UNIX_EPOCH).unwrap();
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await
            .unwrap();
        fixture
    }

    async fn with_orphaned_anchor() -> Self {
        let fixture = Self::new().await;
        fixture
            .workspaces
            .create(fixture.workspace("workspace-1"))
            .await
            .unwrap();
        workspace_runtimes::ActiveModel {
            workspace_id: Set("workspace-1".into()),
            runtime_kind: Set("runpod".into()),
            state: Set("ready".into()),
        }
        .insert(fixture.database.connection())
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

fn runpod_runtime(state: RuntimeState, volume_size_gb: u64) -> Runtime {
    Runtime {
        state,
        provider: RuntimeProvider::Runpod(RunpodRuntime::new_provisioning(RunpodRuntimeConfig {
            datacenter_id: "EU-RO-1".into(),
            gpu_id: "gpu-1".into(),
            volume_size_gb,
        })),
    }
}

fn running_operation(workspace_id: &str, kind: RuntimeOperationKind) -> RuntimeOperation {
    RuntimeOperation::running(
        Uuid::new_v4(),
        workspace_id,
        RuntimeKind::Runpod,
        kind,
        RuntimeProgress::Runpod(RunpodProgress::Provision(
            RunpodProvisionStep::CreateNetworkVolume,
        )),
        OffsetDateTime::UNIX_EPOCH,
    )
}

#[tokio::test]
async fn workspace_hydrates_state_and_runpod_extension_through_dispatch() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);

    fixture
        .transitions
        .save_transition(&workspace, &operation)
        .await
        .unwrap();

    assert_eq!(
        fixture.workspaces.get(&workspace.id).await.unwrap(),
        Some(workspace)
    );
    let anchor = workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(anchor.runtime_kind, "runpod");
    assert_eq!(anchor.state, "provisioning");
    let extension = runpod_workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(extension.volume_size_gb, 100);
}

#[tokio::test]
async fn workspace_page_is_stable_and_reports_total() {
    let fixture = Fixture::new().await;
    let mut expected = fixture.workspace("workspace-2");

    for id in ["workspace-1", "workspace-2", "workspace-3"] {
        let mut workspace = fixture.workspace(id);
        fixture.workspaces.create(workspace.clone()).await.unwrap();
        if id != "workspace-1" {
            workspace.runtime = Some(runpod_runtime(RuntimeState::Ready, 100));
            let operation = running_operation(id, RuntimeOperationKind::Provision);
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
async fn provider_failure_rolls_back_anchor_and_operation() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, u64::MAX));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Provision);

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await,
        Err(RuntimePersistenceError::CorruptData),
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
async fn operation_kind_and_progress_family_mismatch_rolls_back() {
    let fixture = Fixture::new().await;
    let mut workspace = fixture.workspace("workspace-1");
    fixture.workspaces.create(workspace.clone()).await.unwrap();
    workspace.runtime = Some(runpod_runtime(RuntimeState::Provisioning, 100));
    let operation = running_operation(&workspace.id, RuntimeOperationKind::Cleanup);

    assert_eq!(
        fixture
            .transitions
            .save_transition(&workspace, &operation)
            .await,
        Err(RuntimePersistenceError::CorruptData),
    );
    assert!(
        runtime_operations::Entity::find_by_id(operation.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        runpod_runtime_operation_progress::Entity::find_by_id(operation.id.to_string())
            .one(fixture.database.connection())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn cleanup_removes_runtime_but_keeps_dispatched_operation_progress() {
    let fixture = Fixture::with_ready_runtime().await;
    let mut workspace = fixture
        .workspaces
        .get("workspace-1")
        .await
        .unwrap()
        .unwrap();
    workspace.runtime = None;
    let mut operation = running_operation("workspace-1", RuntimeOperationKind::Cleanup);
    operation.progress = RuntimeProgress::Runpod(RunpodProgress::Cleanup(
        luma_forge_lib::application::runtimes::runpod::RunpodCleanupStep::DeleteNetworkVolume,
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
    assert!(runpod_workspace_runtimes::Entity::find_by_id("workspace-1")
        .one(fixture.database.connection())
        .await
        .unwrap()
        .is_none());
    let stored = fixture
        .operations
        .page(Some("workspace-1"), 0, 10)
        .await
        .unwrap()
        .0
        .into_iter()
        .find(|stored| stored.id == operation.id)
        .unwrap();
    assert_eq!(stored.runtime_kind, RuntimeKind::Runpod);
    assert_eq!(stored.progress, operation.progress);
}

#[tokio::test]
async fn anchor_without_provider_extension_is_corrupt() {
    let fixture = Fixture::with_orphaned_anchor().await;
    assert_eq!(
        fixture.workspaces.get("workspace-1").await,
        Err(WorkspaceRepositoryError::CorruptData),
    );
}
