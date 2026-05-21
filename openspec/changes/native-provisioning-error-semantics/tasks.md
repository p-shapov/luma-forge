## 1. Model Error Boundaries

- [x] 1.1 Inventory existing `WorkspaceSetupError`, `WorkspaceResourceError`, `WorkspaceProvisioningError`, `WorkspaceProvisioningFailure`, and `NativeCommandError` variants used by provisioning/resource paths.
- [x] 1.2 Add or adjust `WorkspaceResourceError` variants for catalog/persistence, secret/keyring, provider API, provider resource lifecycle, provider uncertainty, cleanup, and Provisioner Worker token lifecycle categories.
- [x] 1.3 Add or adjust `WorkspaceProvisioningError` variants for workspace identity/state, catalog/persistence, secret/keyring, transient provider/worker, conflict, and escaped resource-operation command failures.
- [x] 1.4 Preserve existing granular Workspace Catalog storage unavailable, migration failed, query failed, corrupt, schema mismatch, and generic unavailable categories through resource and provisioning errors.

## 2. Implement Mapping Semantics

- [x] 2.1 Update Workspace Setup to Workspace Resources conversion helpers to preserve every specific Workspace Catalog category.
- [x] 2.2 Update Workspace Setup to Workspace Provisioning conversion helpers to preserve every specific Workspace Catalog category.
- [x] 2.3 Implement explicit `WorkspaceResourceError -> WorkspaceProvisioningError` mappings for failures that should escape as immediate command errors.
- [x] 2.4 Implement phase-specific handling for resource failures that must become persisted `WorkspaceProvisioningFailure` records.
- [x] 2.5 Ensure catalog/storage/query/migration/corrupt/schema failures return granular command errors and do not mutate Workspace state or provider resources.

## 3. Persist Recovery-Required Failures

- [x] 3.1 Keep provider operation indeterminate states persisted as Workspace failures with cleanup-oriented recovery action when provider state may be unsafe.
- [x] 3.2 Keep tracked provider resource missing states persisted as Workspace failures with cleanup-oriented recovery action.
- [x] 3.3 Keep orphaned provider resource discovery persisted as Workspace failure with stable UI-safe failure metadata.
- [x] 3.4 Keep cancellation cleanup failure persisted as Workspace failure with cleanup-oriented recovery action.
- [x] 3.5 Persist Provisioner Worker unauthorized, invalid response, terminal failure, and token missing/invalid during environment preparation with sanitized worker or native-state failure metadata.
- [x] 3.6 Preserve normal Provisioner Worker startup/readiness lag as running progress without persisted failure or user-facing worker-unavailable command error.

## 4. Harden Token Lifecycle Behavior

- [x] 4.1 Identify the provisioning pod creation path after per-workspace Provisioner Worker token storage succeeds.
- [x] 4.2 Delete the stored worker token best-effort when provisioning pod creation fails with a determinate no-pod-created result.
- [x] 4.3 Preserve the original pod creation failure when best-effort token deletion succeeds or fails.
- [x] 4.4 Avoid deleting the token when provider pod creation is indeterminate or a pod may exist.
- [x] 4.5 Ensure token cleanup, token missing, and token invalid paths never return, persist, or log the token value.

## 5. Stabilize Command Errors

- [x] 5.1 Add or update `NativeCommandErrorCode` mappings for granular catalog, secret/keyring, transient provider, transient worker, conflict, and escaped resource-operation failures.
- [x] 5.2 Ensure command error `message`, `reason`, `retryable`, and `recovery_action` are stable, granular, and implementation-safe.
- [x] 5.3 Ensure persisted `WorkspaceProvisioningFailure` state is returned through Workspace/progress payloads rather than replaced by generic command errors when authoritative Workspace state can be returned.
- [x] 5.4 Regenerate generated TypeScript command bindings if exported command error codes or binding-safe payload types change.

## 6. Regression Tests

- [x] 6.1 Add tests proving each Workspace Catalog category is preserved across Workspace Resources and Workspace Provisioning paths.
- [x] 6.2 Add tests for `WorkspaceResourceError -> WorkspaceProvisioningError` mappings that should escape as command errors.
- [x] 6.3 Add tests for resource errors that should persist `WorkspaceProvisioningFailure` records.
- [x] 6.4 Add tests for `WorkspaceProvisioningError -> NativeCommandError` mapping, including code, reason, retryability, and recovery action.
- [x] 6.5 Add tests for immediate command error versus persisted provisioning failure behavior using the decision matrix cases.
- [x] 6.6 Add provisioning pod token lifecycle tests for determinate create failure cleanup, indeterminate create preservation, and token missing/invalid during environment preparation.
- [x] 6.7 Fix any strict clippy failures in test support, including known type-complexity issues if they block verification.

## 7. Verification

- [x] 7.1 Run `cargo fmt` in `src-tauri`.
- [x] 7.2 Run `cargo test` in `src-tauri`.
- [x] 7.3 Run `cargo clippy --all-targets --all-features -- -D warnings` in `src-tauri`.
- [x] 7.4 Review the completed change against the OpenSpec decision matrix before marking tasks complete.
