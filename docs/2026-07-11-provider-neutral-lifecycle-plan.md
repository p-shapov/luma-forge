# Provider-Neutral Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development and execute this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep lifecycle operation behavior provider-neutral while moving runtime identity and provider progress ownership into `application/runtimes`.

**Architecture:** `LifecycleOperation` stores provider-neutral identity, kind, state, trace, timestamps, and `RuntimeProgress` dispatch. RunPod owns its progress vocabulary under `runtimes/runpod`; workspace stores only an optional runtime kind projection.

**Tech Stack:** Rust 2021, Tokio, SeaORM SQLite, existing application ports/adapters.

## Global Constraints

- Never read or modify `src-tauri/old_src`.
- Add no generic lifecycle type parameter, trait object, DTO, compatibility path, migration, or integration test.
- Keep `Workspace` free of full runtime data; retain `attached_runtime: Option<RuntimeKind>`.
- Keep existing persistence strings, event payloads, transition ordering, detached execution, and public behavior unchanged.
- Use existing unit tests and the smallest fake state needed by each service.
- Every task ends with focused tests, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, and a Conventional Commit.

---

### Task 1: Move Runtime Dispatch Types to Runtime Ownership

**Files:**
- Create: `src-tauri/src/application/runtimes/runpod/progress.rs`
- Delete: `src-tauri/src/application/lifecycle/progress/runpod.rs`
- Modify: `src-tauri/src/application/runtimes/model.rs`
- Modify: `src-tauri/src/application/runtimes/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/mod.rs`
- Modify: `src-tauri/src/application/workspace/model.rs`
- Modify: `src-tauri/src/application/workspace/mod.rs`
- Modify imports in application/adapters that consume `RuntimeKind`.

**Produces:**

```rust
pub enum RuntimeKind {
    Runpod,
}

pub enum RuntimeProgress {
    Runpod(RunpodProgress),
}

pub enum RunpodProgress {
    Provision(RunpodProvisionStep),
    Cleanup(RunpodCleanupStep),
}
```

- [ ] **Step 1: Add a failing runtime-dispatch ownership test**

Extend `application/runtimes/model.rs` with a test that constructs
`RuntimeProgress::Runpod(RunpodProgress::Provision(...))` and asserts
`RuntimeKind::Runpod` remains owned by the runtime module.

- [ ] **Step 2: Run RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib application::runtimes::model::tests
```

Expected: compilation fails before `RuntimeProgress` and the new RunPod progress module exist.

- [ ] **Step 3: Move types and update imports**

Move the three RunPod progress enums unchanged. Define and export
`RuntimeKind`/`RuntimeProgress` from `runtimes/model.rs`; make workspace import
`RuntimeKind` instead of defining/re-exporting it. Update consumers without
changing behavior.

- [ ] **Step 4: Run GREEN and format**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib application::runtimes::model::tests
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(application): move runtime dispatch types"
```

---

### Task 2: Make Lifecycle Operation Behavior Provider-Neutral

**Files:**
- Modify: `src-tauri/src/application/lifecycle/model.rs`
- Modify: `src-tauri/src/application/lifecycle/mod.rs`
- Delete: `src-tauri/src/application/lifecycle/progress/mod.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/service.rs`
- Modify: `src-tauri/src/application/runtimes/runpod/test_support.rs`
- Modify: `src-tauri/src/application/runtimes/transition.rs`
- Modify: `src-tauri/src/adapters/sqlite/lifecycle_operation_repository.rs`
- Modify: `src-tauri/src/adapters/sqlite/runpod_runtime_repository.rs`

**Produces:**

```rust
pub struct LifecycleOperation {
    pub id: Uuid,
    pub workspace_id: String,
    pub kind: LifecycleOperationKind,
    pub state: LifecycleOperationState,
    pub trace_id: Uuid,
    pub progress: RuntimeProgress,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

pub fn running(
    id: Uuid,
    workspace_id: &str,
    trace_id: Uuid,
    kind: LifecycleOperationKind,
    progress: RuntimeProgress,
    now: OffsetDateTime,
) -> Self;

pub fn set_progress(
    &mut self,
    progress: RuntimeProgress,
    now: OffsetDateTime,
) -> Result<(), LifecycleError>;
```

- [ ] **Step 1: Rewrite lifecycle tests against neutral behavior and run RED**

Tests call `running`/`set_progress` and assert explicit `kind`, terminal rules,
trace retention, and progress retention. They must not call any `runpod_*` or
`set_*_step` lifecycle method.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib application::lifecycle::model::tests
```

Expected: compilation fails until the neutral API and explicit `kind` exist.

- [ ] **Step 2: Implement the neutral aggregate and update RunPod callers**

Store `kind` explicitly. Remove provider-specific lifecycle constructors,
setters, accessors, and derived `kind()`. RunPod services construct
`RuntimeProgress::Runpod(...)` and pass it to `running`/`set_progress`.

- [ ] **Step 3: Update SQLite mapping mechanically**

Keep all database strings and tables unchanged. Map persisted RunPod progress
to/from `RuntimeProgress::Runpod`; read/write the explicit `operation.kind`.

- [ ] **Step 4: Run GREEN and format**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib application::lifecycle::model::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib application::runtimes::runpod::service::tests
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(application): neutralize lifecycle operations"
```

---

### Task 3: Remove RunPod Fixtures From Workspace Unit Tests

**Files:**
- Modify: `src-tauri/src/application/workspace/service.rs`
- Modify: `docs/2026-07-11-runtime-background-events-plan.md`

- [ ] **Step 1: Change the workspace fake contract and run RED**

Replace the fake lifecycle operation vector with `has_running: bool`; keep
recorded workspace IDs. Update the running-operation test fixture before its
fake implementation so compilation demonstrates the old fake shape is gone.

- [ ] **Step 2: Implement the minimal fake**

`recent`, `recent_for_workspace`, and `running` return empty vectors;
`has_running` records the workspace ID and returns the configured boolean.
Remove RunPod progress, UUID, and lifecycle fixture imports from workspace tests.

- [ ] **Step 3: Update the existing background-events plan snippets**

Reflect the current ownership paths for `RuntimeKind`, `RuntimeProgress`, and
RunPod progress. Do not change behavior or add another design variant.

- [ ] **Step 4: Run complete verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
rg -n "progress::runpod|runpod_provision|runpod_cleanup|set_provision_step|set_cleanup_step" src-tauri/src/application/lifecycle
rg -n "workspace::RuntimeKind" src-tauri/src
```

Expected: 0 native failures; both `rg` audits return no matches; no files under
`src-tauri/tests`, `src/`, generated contracts, or `old_src` change.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(workspace): use provider-neutral lifecycle fake"
```
