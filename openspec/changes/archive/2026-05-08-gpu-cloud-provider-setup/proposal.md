## Why

LumaForge needs a native-owned provider setup boundary before workspace setup or provisioning can safely call RunPod. The existing flow requires secret handling and provider validation, but the Rust layer currently exposes only demo commands and has no durable provider-key lifecycle.

## What Changes

- Add native commands to read, create, and delete GPU cloud provider setup for the selected provider.
- Store the submitted Provider API Key only in the secure keyring.
- Derive setup status live from keyring presence plus a RunPod identity request.
- Validate submitted keys with a RunPod identity check only; provider permissions are handled later by the flows that require them.
- Reject repeated setup requests after a complete setup already exists.
- Add local setup deletion that removes the keyring entry without revoking the API key in RunPod.
- Add `provider_identity_unavailable` for valid provider authentication where the provider identity or key fingerprint cannot be derived.

## Capabilities

### New Capabilities

- `gpu-cloud-provider-setup`: Native-owned lifecycle for checking, setting once, and deleting the local GPU Cloud Provider API key setup.

### Modified Capabilities

- None.

## Impact

- Affected native code: Tauri command boundary, generated command bindings, provider setup application service, RunPod identity client, keyring-backed secret storage, native error mapping.
- Affected frontend contract: generated TypeScript commands and native response/error shapes; no full React setup UI is included in this change.
- Affected external system: RunPod GraphQL identity API is called for setup status and setup validation.
- Security impact: Provider API Keys remain native-only, are not persisted outside the secure keyring, and are not returned to React.
