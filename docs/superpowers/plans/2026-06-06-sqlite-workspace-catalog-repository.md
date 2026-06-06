# SQLite Workspace Catalog Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an active native `workspace_catalog` module that persists `WorkspaceCatalog` in SQLite through an async repository and pass-through service.

**Architecture:** The new module is isolated under `src-tauri/src/workspace_catalog/`, with focused files for errors, repository trait, service, schema bootstrap, and sqlx SQLite adapter. Workspaces are stored as one JSON row keyed by `Workspace.id`, and the service delegates to the repository without command or lifecycle logic.

**Tech Stack:** Rust 2021, Tauri native backend, `serde_json`, `sqlx` async SQLite, Tokio tests, `time` RFC3339 timestamps, existing `crate::shared::AppFuture`.

---

## File Structure

- Create `src-tauri/src/workspace_catalog/mod.rs`
  - Declares module files and re-exports the public API.

- Create `src-tauri/src/workspace_catalog/errors.rs`
  - Defines `WorkspaceCatalogError`.

- Create `src-tauri/src/workspace_catalog/repository.rs`
  - Defines the async `WorkspaceCatalogRepository` trait.

- Create `src-tauri/src/workspace_catalog/service.rs`
  - Defines `WorkspaceCatalogService<R>` and service delegation tests with a fake repository.

- Create `src-tauri/src/workspace_catalog/schema.rs`
  - Defines schema version constants and bootstrap logic.

- Create `src-tauri/src/workspace_catalog/sqlite.rs`
  - Defines `SqliteWorkspaceCatalogRepository`, `connect(path)`, row serialization/deserialization, and SQLite-backed tests.

- Modify `src-tauri/src/lib.rs`
  - Registers `pub mod workspace_catalog;`.

- Modify `src-tauri/Cargo.toml`
  - Adds `sqlx`, `tokio`, and `time`.

---

### Task 1: Add Dependencies And Module Skeleton

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/workspace_catalog/mod.rs`
- Create: `src-tauri/src/workspace_catalog/errors.rs`
- Create: `src-tauri/src/workspace_catalog/repository.rs`

- [ ] **Step 1: Add async SQLite dependencies**

In `src-tauri/Cargo.toml`, add these entries under `[dependencies]`:

```toml
sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio-rustls"] }
time = { version = "0.3", features = ["formatting"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Register the active module**

In `src-tauri/src/lib.rs`, add the module beside the other active native modules:

```rust
pub mod domain;
pub mod provider_api_key;
pub mod remote_workspace;
pub mod shared;
pub mod workflow_catalog;
pub mod workspace_catalog;
```

- [ ] **Step 3: Create module exports**

Create `src-tauri/src/workspace_catalog/mod.rs`:

```rust
pub mod errors;
pub mod repository;
pub mod schema;
pub mod service;
pub mod sqlite;

pub use errors::WorkspaceCatalogError;
pub use repository::WorkspaceCatalogRepository;
pub use service::WorkspaceCatalogService;
pub use sqlite::SqliteWorkspaceCatalogRepository;
```

- [ ] **Step 4: Create the error type**

Create `src-tauri/src/workspace_catalog/errors.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCatalogError {
    StorageUnavailable,
    MigrationFailed,
    QueryFailed,
    Corrupt,
    SchemaMismatch,
    WorkspaceAlreadyExists,
    WorkspaceNotFound,
}
```

- [ ] **Step 5: Create the repository trait**

Create `src-tauri/src/workspace_catalog/repository.rs`:

```rust
use crate::{
    domain::workspace::{Workspace, WorkspaceCatalog},
    shared::AppFuture,
};

use super::errors::WorkspaceCatalogError;

pub trait WorkspaceCatalogRepository: Send + Sync {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>>;

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>>;

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>>;

    fn update_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>>;

    fn delete_workspace<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>>;
}
```

- [ ] **Step 6: Run native tests to verify the skeleton compiles**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all existing tests compile and pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/workspace_catalog
git commit -m "feat(workspace-catalog): add repository module skeleton"
```

---

### Task 2: Add Pass-Through Service With Delegation Tests

**Files:**
- Create: `src-tauri/src/workspace_catalog/service.rs`

- [ ] **Step 1: Write the service and delegation tests**

Create `src-tauri/src/workspace_catalog/service.rs`:

```rust
use crate::domain::workspace::{Workspace, WorkspaceCatalog};

use super::{errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository};

pub struct WorkspaceCatalogService<R> {
    repository: R,
}

impl<R> WorkspaceCatalogService<R>
where
    R: WorkspaceCatalogRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_workspaces(&self) -> Result<WorkspaceCatalog, WorkspaceCatalogError> {
        self.repository.list_workspaces().await
    }

    pub async fn find_workspace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<Workspace>, WorkspaceCatalogError> {
        self.repository.find_workspace_by_id(id).await
    }

    pub async fn insert_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        self.repository.insert_workspace(workspace).await
    }

    pub async fn update_workspace(
        &self,
        workspace: &Workspace,
    ) -> Result<Workspace, WorkspaceCatalogError> {
        self.repository.update_workspace(workspace).await
    }

    pub async fn delete_workspace(&self, id: &str) -> Result<(), WorkspaceCatalogError> {
        self.repository.delete_workspace(id).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        domain::{
            placement::{
                Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
                RemotePlacementPlan,
            },
            provider::GpuCloudProviderId,
            runtime_contract::RuntimeContractReference,
            workflow_preset::{
                ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
                RemoteRuntimeRequirements, WorkflowExecutionType, WorkflowPreset,
            },
            workspace::{
                RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
                RemoteWorkspaceResources, WorkspaceRuntime,
            },
        },
        shared::AppFuture,
    };

    use super::*;

    struct FakeRepository {
        calls: Arc<Mutex<Vec<&'static str>>>,
        result: Result<(), WorkspaceCatalogError>,
    }

    impl FakeRepository {
        fn new(result: Result<(), WorkspaceCatalogError>) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                result,
            }
        }

        fn calls(&self) -> Arc<Mutex<Vec<&'static str>>> {
            Arc::clone(&self.calls)
        }

        fn result<T>(&self, value: T) -> Result<T, WorkspaceCatalogError> {
            self.result.clone().map(|_| value)
        }
    }

    impl WorkspaceCatalogRepository for FakeRepository {
        fn list_workspaces<'a>(
            &'a self,
        ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("calls lock should succeed")
                    .push("list_workspaces");
                self.result(WorkspaceCatalog {
                    workspaces: vec![workspace("workspace-1")],
                })
            })
        }

        fn find_workspace_by_id<'a>(
            &'a self,
            _id: &'a str,
        ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("calls lock should succeed")
                    .push("find_workspace_by_id");
                self.result(Some(workspace("workspace-1")))
            })
        }

        fn insert_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("calls lock should succeed")
                    .push("insert_workspace");
                self.result(workspace.clone())
            })
        }

        fn update_workspace<'a>(
            &'a self,
            workspace: &'a Workspace,
        ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("calls lock should succeed")
                    .push("update_workspace");
                self.result(workspace.clone())
            })
        }

        fn delete_workspace<'a>(
            &'a self,
            _id: &'a str,
        ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .expect("calls lock should succeed")
                    .push("delete_workspace");
                self.result(())
            })
        }
    }

    fn workspace(id: &str) -> Workspace {
        Workspace {
            id: id.to_string(),
            workflow_preset: WorkflowPreset {
                id: "workflow-1".to_string(),
                version: "1".to_string(),
                name: "Workflow 1".to_string(),
                execution_type: WorkflowExecutionType::T2i,
                requires_hugging_face_api_key: false,
                remote_runtime_requirements: RemoteRuntimeRequirements {
                    required_base_volume_size_bytes: 1,
                    provider_requirements: vec![RemoteProviderRuntimeRequirements {
                        gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                        endpoint_contract: RuntimeContractReference {
                            id: "endpoint-contract".to_string(),
                            version: "1".to_string(),
                        },
                        provisioner_contract: RuntimeContractReference {
                            id: "provisioner-contract".to_string(),
                            version: "1".to_string(),
                        },
                    }],
                },
                required_model_assets: vec![ModelAsset {
                    id: "asset-1".to_string(),
                    name: "Asset 1".to_string(),
                    download_source: ModelAssetSource::Huggingface {
                        repository_id: "owner/repository".to_string(),
                        file_path: "model.safetensors".to_string(),
                        revision: "main".to_string(),
                    },
                    install_comfyui_relative_path: "models/model.safetensors".to_string(),
                }],
            },
            runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
                remote_placement: RemotePlacementPlan {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    datacenter_id: "datacenter-1".to_string(),
                    gpu_id: "gpu-1".to_string(),
                    remote_volume_size_bytes: 1,
                    remote_capabilities: RemotePlacementCapabilities {
                        remote_endpoint_keep_alive: Capability::Supported(
                            RemoteEndpointKeepAliveLimits {
                                default_seconds: 60,
                                min_seconds: 0,
                                max_seconds: 3600,
                            },
                        ),
                    },
                },
                remote_provisioning: RemoteProvisioningState {
                    status: RemoteProvisioningStatus::NotStarted,
                    percent: None,
                },
                remote_resources: RemoteWorkspaceResources {
                    remote_volume: None,
                    remote_provisioner: None,
                    remote_endpoint: None,
                },
            }),
        }
    }

    #[tokio::test]
    async fn list_workspaces_delegates_to_repository() {
        let repository = FakeRepository::new(Ok(()));
        let calls = repository.calls();
        let service = WorkspaceCatalogService::new(repository);

        let catalog = service
            .list_workspaces()
            .await
            .expect("list should succeed");

        assert_eq!(catalog.workspaces, vec![workspace("workspace-1")]);
        assert_eq!(
            *calls.lock().expect("calls lock should succeed"),
            vec!["list_workspaces"]
        );
    }

    #[tokio::test]
    async fn service_preserves_repository_errors() {
        let service = WorkspaceCatalogService::new(FakeRepository::new(Err(
            WorkspaceCatalogError::QueryFailed,
        )));

        assert_eq!(
            service.list_workspaces().await,
            Err(WorkspaceCatalogError::QueryFailed)
        );
        assert_eq!(
            service.find_workspace_by_id("workspace-1").await,
            Err(WorkspaceCatalogError::QueryFailed)
        );
        assert_eq!(
            service.insert_workspace(&workspace("workspace-1")).await,
            Err(WorkspaceCatalogError::QueryFailed)
        );
        assert_eq!(
            service.update_workspace(&workspace("workspace-1")).await,
            Err(WorkspaceCatalogError::QueryFailed)
        );
        assert_eq!(
            service.delete_workspace("workspace-1").await,
            Err(WorkspaceCatalogError::QueryFailed)
        );
    }
}
```

- [ ] **Step 2: Run service tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog::service
```

Expected: service tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/workspace_catalog/service.rs
git commit -m "feat(workspace-catalog): add service boundary"
```

---

### Task 3: Add Schema Bootstrap And SQLite Connect

**Files:**
- Create: `src-tauri/src/workspace_catalog/schema.rs`
- Create: `src-tauri/src/workspace_catalog/sqlite.rs`

- [ ] **Step 1: Add schema bootstrap**

Create `src-tauri/src/workspace_catalog/schema.rs`:

```rust
use sqlx::{Executor, SqlitePool};

use super::errors::WorkspaceCatalogError;

const SCHEMA_VERSION_KEY: &str = "workspace_catalog_schema_version";
const SCHEMA_VERSION: &str = "1";

pub async fn bootstrap(pool: &SqlitePool) -> Result<(), WorkspaceCatalogError> {
    pool.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    pool.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id TEXT PRIMARY KEY,
            workspace_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await
    .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    sqlx::query("INSERT OR IGNORE INTO metadata (key, value) VALUES (?1, ?2)")
        .bind(SCHEMA_VERSION_KEY)
        .bind(SCHEMA_VERSION)
        .execute(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    let version: Option<String> = sqlx::query_scalar("SELECT value FROM metadata WHERE key = ?1")
        .bind(SCHEMA_VERSION_KEY)
        .fetch_optional(pool)
        .await
        .map_err(|_| WorkspaceCatalogError::MigrationFailed)?;

    match version.as_deref() {
        Some(SCHEMA_VERSION) => Ok(()),
        _ => Err(WorkspaceCatalogError::SchemaMismatch),
    }
}
```

- [ ] **Step 2: Add SQLite repository connect**

Create `src-tauri/src/workspace_catalog/sqlite.rs` with the connect shell and schema creation test:

```rust
use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use super::{errors::WorkspaceCatalogError, schema};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceCatalogError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|_| WorkspaceCatalogError::StorageUnavailable)?;

        schema::bootstrap(&pool).await?;

        Ok(Self { pool })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use sqlx::Row;

    use super::*;

    fn catalog_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("luma-forge-{name}-{nonce}.sqlite"))
    }

    async fn table_exists(pool: &SqlitePool, table_name: &str) -> bool {
        let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1")
            .bind(table_name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master query should succeed");

        row.is_some()
    }

    #[tokio::test]
    async fn connect_creates_schema() {
        let path = catalog_path("schema");

        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should create schema");

        assert!(table_exists(&repository.pool, "metadata").await);
        assert!(table_exists(&repository.pool, "workspaces").await);

        let version = sqlx::query("SELECT value FROM metadata WHERE key = ?1")
            .bind("workspace_catalog_schema_version")
            .fetch_one(&repository.pool)
            .await
            .expect("metadata version should exist")
            .get::<String, _>("value");

        assert_eq!(version, "1");

        drop(repository);
        let _ = fs::remove_file(path);
    }
}
```

- [ ] **Step 3: Run schema test**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog::sqlite::tests::connect_creates_schema
```

Expected: `connect_creates_schema` passes.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/workspace_catalog/schema.rs src-tauri/src/workspace_catalog/sqlite.rs
git commit -m "feat(workspace-catalog): bootstrap sqlite schema"
```

---

### Task 4: Implement List, Find, And Insert

**Files:**
- Modify: `src-tauri/src/workspace_catalog/sqlite.rs`

- [ ] **Step 1: Add repository implementation and test fixture helpers**

Extend the production section of `src-tauri/src/workspace_catalog/sqlite.rs` with the repository implementation below. Keep the schema test module from Task 3, then add the additional tests in Step 2.

```rust
use std::path::Path;

use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    domain::workspace::{Workspace, WorkspaceCatalog},
    shared::{is_blank, AppFuture},
};

use super::{
    errors::WorkspaceCatalogError, repository::WorkspaceCatalogRepository, schema,
};

#[derive(Debug, Clone)]
pub struct SqliteWorkspaceCatalogRepository {
    pool: SqlitePool,
}

impl SqliteWorkspaceCatalogRepository {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self, WorkspaceCatalogError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|_| WorkspaceCatalogError::StorageUnavailable)?;

        schema::bootstrap(&pool).await?;

        Ok(Self { pool })
    }
}

impl WorkspaceCatalogRepository for SqliteWorkspaceCatalogRepository {
    fn list_workspaces<'a>(
        &'a self,
    ) -> AppFuture<'a, Result<WorkspaceCatalog, WorkspaceCatalogError>> {
        Box::pin(async move {
            let rows = sqlx::query("SELECT id, workspace_json FROM workspaces ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await
                .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

            let mut workspaces = Vec::with_capacity(rows.len());
            for row in rows {
                workspaces.push(read_workspace_row(&row)?);
            }

            Ok(WorkspaceCatalog { workspaces })
        })
    }

    fn find_workspace_by_id<'a>(
        &'a self,
        id: &'a str,
    ) -> AppFuture<'a, Result<Option<Workspace>, WorkspaceCatalogError>> {
        Box::pin(async move {
            validate_id(id)?;

            let row = sqlx::query("SELECT id, workspace_json FROM workspaces WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

            row.as_ref().map(read_workspace_row).transpose()
        })
    }

    fn insert_workspace<'a>(
        &'a self,
        workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async move {
            validate_id(&workspace.id)?;
            let workspace_json =
                serde_json::to_string(workspace).map_err(|_| WorkspaceCatalogError::Corrupt)?;
            let now = timestamp()?;

            let result = sqlx::query(
                "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(&workspace.id)
            .bind(workspace_json)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => Ok(workspace.clone()),
                Err(error) if is_unique_constraint(&error) => {
                    Err(WorkspaceCatalogError::WorkspaceAlreadyExists)
                }
                Err(_) => Err(WorkspaceCatalogError::QueryFailed),
            }
        })
    }

    fn update_workspace<'a>(
        &'a self,
        _workspace: &'a Workspace,
    ) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
        Box::pin(async { Err(WorkspaceCatalogError::QueryFailed) })
    }

    fn delete_workspace<'a>(
        &'a self,
        _id: &'a str,
    ) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
        Box::pin(async { Err(WorkspaceCatalogError::QueryFailed) })
    }
}

fn validate_id(id: &str) -> Result<(), WorkspaceCatalogError> {
    if is_blank(id) {
        return Err(WorkspaceCatalogError::Corrupt);
    }

    Ok(())
}

fn read_workspace_row(row: &sqlx::sqlite::SqliteRow) -> Result<Workspace, WorkspaceCatalogError> {
    let id: String = row
        .try_get("id")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workspace_json: String = row
        .try_get("workspace_json")
        .map_err(|_| WorkspaceCatalogError::SchemaMismatch)?;
    let workspace: Workspace =
        serde_json::from_str(&workspace_json).map_err(|_| WorkspaceCatalogError::Corrupt)?;

    if workspace.id != id {
        return Err(WorkspaceCatalogError::Corrupt);
    }

    Ok(workspace)
}

fn timestamp() -> Result<String, WorkspaceCatalogError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| WorkspaceCatalogError::QueryFailed)
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "1555" || code == "2067")
}
```

- [ ] **Step 2: Add list/find/insert tests**

Inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/workspace_catalog/sqlite.rs`, keep the `catalog_path` helper from Task 3. Add these imports to the test module:

```rust
use crate::domain::{
    placement::{
        Capability, RemoteEndpointKeepAliveLimits, RemotePlacementCapabilities,
        RemotePlacementPlan,
    },
    provider::GpuCloudProviderId,
    runtime_contract::RuntimeContractReference,
    workflow_preset::{
        ModelAsset, ModelAssetSource, RemoteProviderRuntimeRequirements,
        RemoteRuntimeRequirements, WorkflowExecutionType, WorkflowPreset,
    },
    workspace::{
        RemoteProvisioningState, RemoteProvisioningStatus, RemoteWorkspace,
        RemoteWorkspaceResources, Workspace, WorkspaceCatalog, WorkspaceRuntime,
    },
};
```

Add this fixture helper to the test module:

```rust
fn workspace(id: &str) -> Workspace {
    Workspace {
        id: id.to_string(),
        workflow_preset: WorkflowPreset {
            id: "workflow-1".to_string(),
            version: "1".to_string(),
            name: "Workflow 1".to_string(),
            execution_type: WorkflowExecutionType::T2i,
            requires_hugging_face_api_key: false,
            remote_runtime_requirements: RemoteRuntimeRequirements {
                required_base_volume_size_bytes: 1,
                provider_requirements: vec![RemoteProviderRuntimeRequirements {
                    gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                    endpoint_contract: RuntimeContractReference {
                        id: "endpoint-contract".to_string(),
                        version: "1".to_string(),
                    },
                    provisioner_contract: RuntimeContractReference {
                        id: "provisioner-contract".to_string(),
                        version: "1".to_string(),
                    },
                }],
            },
            required_model_assets: vec![ModelAsset {
                id: "asset-1".to_string(),
                name: "Asset 1".to_string(),
                download_source: ModelAssetSource::Huggingface {
                    repository_id: "owner/repository".to_string(),
                    file_path: "model.safetensors".to_string(),
                    revision: "main".to_string(),
                },
                install_comfyui_relative_path: "models/model.safetensors".to_string(),
            }],
        },
        runtime: WorkspaceRuntime::Remote(RemoteWorkspace {
            remote_placement: RemotePlacementPlan {
                gpu_cloud_provider_id: GpuCloudProviderId::Runpod,
                datacenter_id: "datacenter-1".to_string(),
                gpu_id: "gpu-1".to_string(),
                remote_volume_size_bytes: 1,
                remote_capabilities: RemotePlacementCapabilities {
                    remote_endpoint_keep_alive: Capability::Supported(
                        RemoteEndpointKeepAliveLimits {
                            default_seconds: 60,
                            min_seconds: 0,
                            max_seconds: 3600,
                        },
                    ),
                },
            },
            remote_provisioning: RemoteProvisioningState {
                status: RemoteProvisioningStatus::NotStarted,
                percent: None,
            },
            remote_resources: RemoteWorkspaceResources {
                remote_volume: None,
                remote_provisioner: None,
                remote_endpoint: None,
            },
        }),
    }
}
```

Then append these tests:

```rust
#[tokio::test]
async fn list_workspaces_returns_empty_catalog() {
    let path = catalog_path("empty");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");

    let catalog = repository
        .list_workspaces()
        .await
        .expect("list should succeed");

    assert_eq!(catalog, WorkspaceCatalog { workspaces: vec![] });

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn insert_list_and_find_round_trip_workspace() {
    let path = catalog_path("round-trip");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let workspace = workspace("workspace-1");

    let inserted = repository
        .insert_workspace(&workspace)
        .await
        .expect("insert should succeed");
    let catalog = repository
        .list_workspaces()
        .await
        .expect("list should succeed");
    let found = repository
        .find_workspace_by_id("workspace-1")
        .await
        .expect("find should succeed");

    assert_eq!(inserted, workspace);
    assert_eq!(catalog.workspaces, vec![workspace.clone()]);
    assert_eq!(found, Some(workspace));

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn duplicate_insert_returns_workspace_already_exists() {
    let path = catalog_path("duplicate");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let workspace = workspace("workspace-1");

    repository
        .insert_workspace(&workspace)
        .await
        .expect("initial insert should succeed");

    assert_eq!(
        repository.insert_workspace(&workspace).await,
        Err(WorkspaceCatalogError::WorkspaceAlreadyExists)
    );

    drop(repository);
    let _ = fs::remove_file(path);
}
```

- [ ] **Step 3: Run list/find/insert tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog::sqlite
```

Expected: the SQLite repository tests written so far pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/workspace_catalog/sqlite.rs
git commit -m "feat(workspace-catalog): persist workspace rows"
```

---

### Task 5: Implement Update And Delete

**Files:**
- Modify: `src-tauri/src/workspace_catalog/sqlite.rs`

- [ ] **Step 1: Replace update and delete implementations**

In `src-tauri/src/workspace_catalog/sqlite.rs`, replace the temporary `update_workspace` and `delete_workspace` methods with:

```rust
fn update_workspace<'a>(
    &'a self,
    workspace: &'a Workspace,
) -> AppFuture<'a, Result<Workspace, WorkspaceCatalogError>> {
    Box::pin(async move {
        validate_id(&workspace.id)?;
        let workspace_json =
            serde_json::to_string(workspace).map_err(|_| WorkspaceCatalogError::Corrupt)?;
        let now = timestamp()?;

        let result = sqlx::query(
            "UPDATE workspaces
            SET workspace_json = ?1, updated_at = ?2
            WHERE id = ?3",
        )
        .bind(workspace_json)
        .bind(now)
        .bind(&workspace.id)
        .execute(&self.pool)
        .await
        .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

        if result.rows_affected() == 0 {
            return Err(WorkspaceCatalogError::WorkspaceNotFound);
        }

        Ok(workspace.clone())
    })
}

fn delete_workspace<'a>(
    &'a self,
    id: &'a str,
) -> AppFuture<'a, Result<(), WorkspaceCatalogError>> {
    Box::pin(async move {
        validate_id(id)?;

        let result = sqlx::query("DELETE FROM workspaces WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|_| WorkspaceCatalogError::QueryFailed)?;

        if result.rows_affected() == 0 {
            return Err(WorkspaceCatalogError::WorkspaceNotFound);
        }

        Ok(())
    })
}
```

- [ ] **Step 2: Add update and delete tests**

Append these tests in `src-tauri/src/workspace_catalog/sqlite.rs`:

```rust
#[tokio::test]
async fn update_replaces_existing_workspace() {
    let path = catalog_path("update");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let initial = workspace("workspace-1");
    let mut updated = workspace("workspace-1");
    updated.workflow_preset.name = "Updated Workflow".to_string();

    repository
        .insert_workspace(&initial)
        .await
        .expect("insert should succeed");
    let persisted = repository
        .update_workspace(&updated)
        .await
        .expect("update should succeed");
    let found = repository
        .find_workspace_by_id("workspace-1")
        .await
        .expect("find should succeed");

    assert_eq!(persisted, updated);
    assert_eq!(found, Some(updated));

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn delete_removes_existing_workspace() {
    let path = catalog_path("delete");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let workspace = workspace("workspace-1");

    repository
        .insert_workspace(&workspace)
        .await
        .expect("insert should succeed");
    repository
        .delete_workspace("workspace-1")
        .await
        .expect("delete should succeed");

    assert_eq!(
        repository
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed"),
        None
    );

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn missing_update_returns_workspace_not_found() {
    let path = catalog_path("missing-update");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");

    assert_eq!(
        repository.update_workspace(&workspace("workspace-1")).await,
        Err(WorkspaceCatalogError::WorkspaceNotFound)
    );

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn missing_delete_returns_workspace_not_found() {
    let path = catalog_path("missing-delete");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");

    assert_eq!(
        repository.delete_workspace("workspace-1").await,
        Err(WorkspaceCatalogError::WorkspaceNotFound)
    );

    drop(repository);
    let _ = fs::remove_file(path);
}
```

- [ ] **Step 3: Run update and delete tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog::sqlite
```

Expected: the SQLite repository tests written so far pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/workspace_catalog/sqlite.rs
git commit -m "feat(workspace-catalog): update and delete workspaces"
```

---

### Task 6: Add Corruption, Ordering, And Reconnect Coverage

**Files:**
- Modify: `src-tauri/src/workspace_catalog/sqlite.rs`

- [ ] **Step 1: Add integrity and reconnect tests**

Append these tests in `src-tauri/src/workspace_catalog/sqlite.rs`:

```rust
#[tokio::test]
async fn corrupt_workspace_json_returns_corrupt() {
    let path = catalog_path("corrupt-json");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");

    sqlx::query(
        "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4)",
    )
    .bind("workspace-1")
    .bind("{")
    .bind("2026-06-06T00:00:00Z")
    .bind("2026-06-06T00:00:00Z")
    .execute(&repository.pool)
    .await
    .expect("manual insert should succeed");

    assert_eq!(
        repository.list_workspaces().await,
        Err(WorkspaceCatalogError::Corrupt)
    );

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn row_id_workspace_id_mismatch_returns_corrupt() {
    let path = catalog_path("id-mismatch");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let workspace_json =
        serde_json::to_string(&workspace("workspace-json")).expect("workspace should serialize");

    sqlx::query(
        "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4)",
    )
    .bind("workspace-row")
    .bind(workspace_json)
    .bind("2026-06-06T00:00:00Z")
    .bind("2026-06-06T00:00:00Z")
    .execute(&repository.pool)
    .await
    .expect("manual insert should succeed");

    assert_eq!(
        repository.find_workspace_by_id("workspace-row").await,
        Err(WorkspaceCatalogError::Corrupt)
    );

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn list_workspaces_orders_by_created_at() {
    let path = catalog_path("ordering");
    let repository = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("connect should succeed");
    let first = workspace("workspace-1");
    let second = workspace("workspace-2");

    sqlx::query(
        "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4)",
    )
    .bind("workspace-2")
    .bind(serde_json::to_string(&second).expect("workspace should serialize"))
    .bind("2026-06-06T00:00:02Z")
    .bind("2026-06-06T00:00:02Z")
    .execute(&repository.pool)
    .await
    .expect("manual insert should succeed");

    sqlx::query(
        "INSERT INTO workspaces (id, workspace_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4)",
    )
    .bind("workspace-1")
    .bind(serde_json::to_string(&first).expect("workspace should serialize"))
    .bind("2026-06-06T00:00:01Z")
    .bind("2026-06-06T00:00:01Z")
    .execute(&repository.pool)
    .await
    .expect("manual insert should succeed");

    let catalog = repository
        .list_workspaces()
        .await
        .expect("list should succeed");

    assert_eq!(catalog.workspaces, vec![first, second]);

    drop(repository);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn persisted_workspace_survives_reconnect() {
    let path = catalog_path("reconnect");
    let workspace = workspace("workspace-1");

    {
        let repository = SqliteWorkspaceCatalogRepository::connect(&path)
            .await
            .expect("connect should succeed");
        repository
            .insert_workspace(&workspace)
            .await
            .expect("insert should succeed");
    }

    let reconnected = SqliteWorkspaceCatalogRepository::connect(&path)
        .await
        .expect("reconnect should succeed");

    assert_eq!(
        reconnected
            .find_workspace_by_id("workspace-1")
            .await
            .expect("find should succeed"),
        Some(workspace)
    );

    drop(reconnected);
    let _ = fs::remove_file(path);
}
```

- [ ] **Step 2: Run integrity and reconnect tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml workspace_catalog::sqlite
```

Expected: all SQLite repository tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/workspace_catalog/sqlite.rs
git commit -m "test(workspace-catalog): cover sqlite integrity cases"
```

---

### Task 7: Run Full Verification

**Files:**
- No new files.

- [ ] **Step 1: Format the native crate**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml
```

Expected: command exits successfully.

- [ ] **Step 2: Run all native tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all tests pass.

- [ ] **Step 3: Check formatting**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected: command exits successfully without diffs.

- [ ] **Step 4: Run clippy**

Run:

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Expected: clippy exits successfully with no warnings.

- [ ] **Step 5: Confirm command codegen is not needed**

Run:

```bash
git diff -- src/generated/commands.ts
```

Expected: no output. This slice does not change Tauri commands or generated frontend bindings.

- [ ] **Step 6: Commit final verification formatting if needed**

If `cargo fmt` changed files, commit them:

```bash
git add src-tauri
git commit -m "style(workspace-catalog): format sqlite repository"
```

If `cargo fmt` did not change files, do not create an empty commit.

---

## Self-Review Notes

- Spec coverage: The plan covers module creation, dependencies, schema bootstrap, repository trait, service boundary, SQLite connect, list/find/insert/update/delete operations, duplicate/missing/corrupt/reconnect behavior, ordering, and native verification.
- Scope check: No Tauri command wiring, codegen, frontend changes, legacy migration, normalized schema, app data path wiring, or nested domain validation are included.
- Ambiguity resolved: `WorkspaceNotFound` is included as a dedicated `WorkspaceCatalogError` variant and is used for missing update/delete rows.
