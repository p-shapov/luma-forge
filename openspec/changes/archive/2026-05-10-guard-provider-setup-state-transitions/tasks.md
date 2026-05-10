## 1. Native Coordination Boundary

- [x] 1.1 Add a provider-keyed native coordinator for GPU Cloud Provider setup operations.
- [x] 1.2 Register the coordinator as shared Tauri application state.
- [x] 1.3 Inject the coordinator into provider setup commands that mutate local setup state.
- [x] 1.4 Inject the coordinator into Workspace creation for provider setup prerequisite protection.

## 2. Provider Setup Behavior

- [x] 2.1 Serialize `setup_gpu_cloud_provider` for the requested provider across existing-key read, provider validation, keyring write, and response derivation.
- [x] 2.2 Re-read the stored Provider API Key after setup writes and derive the returned `GpuCloudProviderSetup` from that stored key.
- [x] 2.3 Ensure a later concurrent setup request rejects with `provider_setup_already_exists` before validating its submitted key.
- [x] 2.4 Serialize `delete_gpu_cloud_provider_setup` for the requested provider across keyring read and delete.
- [x] 2.5 Preserve existing provider setup command DTOs, generated frontend bindings, and UI-safe error code mapping.

## 3. Workspace Setup Behavior

- [x] 3.1 Serialize `create_workspace` with provider setup deletion for the request provider.
- [x] 3.2 Hold provider setup protection from provider key prerequisite validation through Workspace Catalog insert and re-read.
- [x] 3.3 Preserve SQLite primary-key handling as the authoritative duplicate Workspace UUID boundary.
- [x] 3.4 Preserve existing Workspace Setup command DTOs, generated frontend bindings, and UI-safe error code mapping.

## 4. Tests

- [x] 4.1 Add provider setup tests proving concurrent setup requests produce one success and one `provider_setup_already_exists`.
- [x] 4.2 Add provider setup tests proving the losing concurrent setup request does not validate its submitted key after setup exists.
- [x] 4.3 Add provider setup tests proving setup success is derived from the re-read stored key and maps re-read or validation failure correctly.
- [x] 4.4 Add provider setup tests for setup/delete serialization order.
- [x] 4.5 Add Workspace Setup tests for Workspace creation racing with provider setup deletion.
- [x] 4.6 Add or preserve Workspace Catalog tests proving duplicate Workspace UUID handling still comes from SQLite uniqueness.

## 5. Verification

- [x] 5.1 Run `cargo test`.
- [x] 5.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.3 Run `cargo fmt`.
- [x] 5.4 Confirm generated TypeScript command bindings are unchanged unless command state injection requires non-contract regeneration.
