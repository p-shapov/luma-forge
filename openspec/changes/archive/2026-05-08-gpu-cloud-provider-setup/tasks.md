## 1. Native Contract and Module Structure

- [x] 1.1 Replace demo command exports with provider setup command types for get, setup, and delete.
- [x] 1.2 Add `provider_identity_unavailable` to the native error code mapping.
- [x] 1.3 Create native module boundaries for provider setup service, provider identity clients, RunPod adapter, and keyring-backed secret storage.
- [x] 1.4 Regenerate TypeScript command bindings and verify no Provider API Key appears in response types.

## 2. Provider Identity and Secret Storage

- [x] 2.1 Implement a keyring-backed secret store for provider-scoped API key read, replace, and delete operations.
- [x] 2.2 Implement the provider registry for supported GPU Cloud Provider ids with v1 RunPod-only resolution.
- [x] 2.3 Implement the RunPod identity request using the submitted or stored Provider API Key.
- [x] 2.4 Implement RunPod API key fingerprint derivation by matching exactly one `apiKeys[].id` prefix with `isActive == true`.
- [x] 2.5 Map invalid keys, inactive matched keys, provider/network failures, keyring failures, and identity-derivation failures to the agreed native error codes.

## 3. Provider Setup Service

- [x] 3.1 Implement live status lookup: missing key returns nullable setup, stored key triggers provider identity validation.
- [x] 3.2 Implement one-time setup: reject existing setup before validation, validate submitted key before keyring mutation, re-read after write, and return live setup.
- [x] 3.3 Preserve existing setup by rejecting repeated setup without mutating the keyring.
- [x] 3.4 Implement non-idempotent delete: missing setup returns `provider_setup_incomplete`, existing setup deletes only the local keyring entry.
- [x] 3.5 Ensure service responses and errors never expose Provider API Key values.

## 4. Command Wiring

- [x] 4.1 Wire `get_gpu_cloud_provider_setup` to the provider setup service.
- [x] 4.2 Wire `setup_gpu_cloud_provider` to the provider setup service.
- [x] 4.3 Wire `delete_gpu_cloud_provider_setup` to the provider setup service.
- [x] 4.4 Register commands in the Tauri Specta builder and remove obsolete demo commands if no longer needed.

## 5. Tests and Verification

- [x] 5.1 Add service tests for missing key, valid stored key, invalid key, inactive key, identity mismatch, and provider API unavailable cases.
- [x] 5.2 Add setup tests proving repeated setup is rejected before validation, invalid setup does not mutate keyring, and write failure is reported.
- [x] 5.3 Add delete tests for successful delete, missing setup error, and keyring failure.
- [x] 5.4 Add RunPod adapter tests for GraphQL response parsing and prefix-match ambiguity handling.
- [x] 5.5 Run `cargo test`.
- [x] 5.6 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.7 Run `cargo fmt`.
