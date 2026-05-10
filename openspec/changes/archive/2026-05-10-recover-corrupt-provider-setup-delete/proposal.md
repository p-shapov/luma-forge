## Why

Deleting local GPU Cloud Provider setup currently depends on reading and parsing the stored Provider API Key before deletion. If the keyring entry exists but contains corrupt or otherwise unparsable key material, the exposed delete flow fails before removing it, leaving the user without a provider-specific recovery path until Factory Reset exists.

## What Changes

- Treat delete as an operation on the local keyring entry, not on a domain-valid Provider API Key.
- Allow `delete_gpu_cloud_provider_setup` to remove a present provider keyring entry even when the stored value cannot be parsed as a valid Provider API Key.
- Preserve the existing behavior that deleting a missing setup returns `provider_setup_incomplete`.
- Keep status, setup creation, workspace setup, and provider calls strict: flows that need a usable Provider API Key still reject corrupt stored key material.
- Do not add provider-side API key revocation or a full Factory Reset flow in this change.

## Capabilities

### New Capabilities

### Modified Capabilities
- `gpu-cloud-provider-setup`: delete behavior changes to recover from a present but unusable local provider keyring entry without requiring Factory Reset.

## Impact

- Affects native provider setup deletion in `src-tauri/src/provider_setup/`.
- Affects secure keyring abstraction behavior in `src-tauri/src/secrets/`.
- Adds tests for deleting corrupt local provider setup state.
- No generated command request or response shape changes are expected.
- No frontend API changes are expected beyond receiving successful delete for the corrupt-entry case.
