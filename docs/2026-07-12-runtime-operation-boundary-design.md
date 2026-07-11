# Runtime Operation Boundary Design

## Goal

Remove the standalone `application/lifecycle` boundary. The current lifecycle model and repository exist only to describe and query runtime Provision/Cleanup operations, so they belong to `application/runtimes`.

## Application structure

`application/runtimes` owns:

- `Runtime`, `RuntimeKind`, and `RuntimeProgress`;
- `RuntimeOperation`, `RuntimeOperationKind`, and `RuntimeOperationState`;
- runtime operation transition validation;
- `RuntimeOperationRepository` for operation journal reads;
- `RuntimeTransitionRepository` for atomic runtime and operation writes;
- provider-specific runtime implementations and progress types.

The operation model moves to `application/runtimes/operation.rs`. Its repository port moves to `application/runtimes/ports/runtime_operation_repository.rs`. The standalone `application/lifecycle` module is deleted.

Ports remain inside their owning application boundary. There is no shared `application/ports` directory because workspace, secret, runtime, and provider ports do not form one cohesive boundary.

## Vocabulary

The refactor uses runtime-operation vocabulary consistently:

- `LifecycleOperation` becomes `RuntimeOperation`;
- `LifecycleOperationKind` becomes `RuntimeOperationKind`;
- `LifecycleOperationState` becomes `RuntimeOperationState`;
- `LifecycleError` becomes `RuntimeOperationError`;
- `LifecycleOperationRepository` becomes `RuntimeOperationRepository`;
- `ApplicationEvent::LifecycleOperationChanged` becomes `ApplicationEvent::RuntimeOperationChanged`;
- dependency and fake fields named `lifecycle` become `operations`.

The unused `OperationAlreadyRunning` variant is removed from the operation transition error. Persistence continues to enforce the one-running-operation invariant and reports it through `RuntimeTransitionRepositoryError::OperationAlreadyRunning`.

## Workspace projection

`Workspace::attached_runtime` becomes `Workspace::runtime` and remains `Option<RuntimeKind>`. It is only a provider-kind projection used to determine whether a workspace has a runtime. The full provider runtime is not embedded in the workspace model.

## Persistence

The current pre-v1 schema is renamed directly:

- `lifecycle_operations` becomes `runtime_operations`;
- `runpod_lifecycle_progress` becomes `runpod_runtime_operation_progress`;
- the SQLite operation repository adapter and SeaORM entity modules use the same runtime-operation vocabulary.

All current callers, relations, schema creation, adapters, and tests are updated together. No migration, compatibility alias, or fallback is added.

Runtime and operation state remain separate models. `RuntimeTransitionRepository` continues to persist both atomically; moving the operation under the runtime boundary does not combine their state machines.

## Events and behavior

Provision, cleanup, interrupted-operation recovery, transition ordering, trace IDs, and emitted payloads retain their current behavior. Only ownership and vocabulary change.

The event order remains:

1. commit the runtime and operation transition;
2. emit the workspace projection when attachment changes;
3. emit the runtime change or deletion;
4. emit `RuntimeOperationChanged`.

## Verification

Existing behavior tests are renamed with the production types. No new integration-test layer or compatibility assertions are added. Verification runs:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

A final search confirms that active Rust code contains no `application::lifecycle`, `LifecycleOperation`, or `attached_runtime` vocabulary.
