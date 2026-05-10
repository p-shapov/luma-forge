## Why

GPU Cloud Provider setup can currently return failure after writing the submitted Provider API Key to the secure keyring when final re-read or stored-key identity validation fails. That leaves the user in a hidden partial state where retry may be rejected as `provider_setup_already_exists`, contradicting setup retry expectations after transient provider or keyring failures.

## What Changes

- Roll back the newly written provider keyring entry when setup finalization fails after a first-time write.
- Preserve the existing invariant that setup success is reported only after the stored key is re-read and validated.
- Return the original finalization error when rollback succeeds, so normal transient failures remain retryable.
- **BREAKING**: add a distinct native command error code for rollback failure after setup finalization fails, so the UI can distinguish "retry after cleanup" from "local recovery required".
- Keep repeated setup rejected after a complete setup exists; this change does not add key rotation or provider-side key revocation.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gpu-cloud-provider-setup`: setup finalization failures after a first-time keyring write must either remove the newly written key before returning the finalization error, or return a distinct recovery-required error if rollback cannot remove the key.
- `native-boundaries`: the generated native command error contract gains a recovery-required provider setup error code for failed rollback after partial setup finalization.

## Impact

- Affects native provider setup creation in `src-tauri/src/provider_setup/`.
- Affects the secure keyring mutation/error path through the existing `SecretStore` abstraction.
- Affects generated command error codes and any frontend error handling that exhaustively matches `NativeCommandErrorCode`.
- Does not add setup-specific frontend error presentation in this slice because the current React app has no Provider Setup UI error path.
- Adds provider setup tests for rollback on post-write re-read failure, rollback on post-write provider validation failure, and rollback failure mapping.
- No storage schema migration is expected.
- No command request or success response shape changes are expected.
