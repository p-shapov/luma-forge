## 1. Native Architecture and Persistence

- [x] 1.1 Add native module structure for domain, application services, infrastructure repositories, provider implementations, and provider setup commands.
- [x] 1.2 Define domain types for GPU Cloud Provider id, redacted provider setup, setup request validation, provider setup errors, and UI-safe command errors.
- [x] 1.3 Add SQLite database initialization and migration support for mutable user data if not already present.
- [x] 1.4 Add SQLite provider setup metadata schema with uniqueness constraints for the selected provider.
- [x] 1.5 Implement a provider setup repository for saving, reading, and verifying redacted provider setup metadata.

## 2. Secret Storage

- [x] 2.1 Implement a keyring-backed Provider API Key store with one key entry per GPU Cloud Provider.
- [x] 2.2 Ensure Provider API Key values use secret-handling types and are not exposed through debug output, errors, logs, responses, or generated frontend types.
- [x] 2.3 Implement keyring read, write, presence check, and best-effort delete operations needed by setup and rollback.

## 3. GPU Provider Abstraction

- [x] 3.1 Define a narrow `GpuProvider` interface with `validate_api_key` returning provider user id and provider API key fingerprint.
- [x] 3.2 Add a provider registry that resolves supported provider ids and rejects unsupported providers.
- [x] 3.3 Implement `RunPodProvider` using a GraphQL `myself` identity request.
- [x] 3.4 Match the submitted RunPod key to an active returned API key id and use that id as `provider_api_key_fingerprint`.
- [x] 3.5 Map invalid credentials, provider API failures, timeouts, and malformed provider responses to UI-safe native errors.

## 4. Provider Setup Application Service

- [x] 4.1 Implement local-only complete setup status reads from SQLite metadata plus keyring presence.
- [x] 4.2 Implement provider-backed recovery when keyring contains a provider key but SQLite metadata is missing.
- [x] 4.3 Implement setup submission ordering: validate request, reject existing complete setup, validate provider key, write keyring, persist SQLite metadata, re-read status, return redacted setup.
- [x] 4.4 Implement fail-closed rollback behavior when SQLite metadata persistence fails after keyring write.
- [x] 4.5 Serialize setup status recovery and setup submission to prevent concurrent duplicate completion.

## 5. Tauri Commands and Generated Bindings

- [x] 5.1 Add `get_gpu_cloud_provider_setup` and `setup_gpu_cloud_provider` Tauri command adapters over the application service.
- [x] 5.2 Export request, response, and error types through `specta` / `tauri-specta`.
- [x] 5.3 Remove or isolate placeholder command usage where it conflicts with the real provider setup command builder.
- [x] 5.4 Regenerate `src/generated/commands.ts`.

## 6. Frontend Integration

- [x] 6.1 Add or update frontend command access for provider setup using generated bindings.
- [x] 6.2 Ensure React treats returned provider setup status as authoritative and never stores or re-exposes Provider API Key values after submission.
- [x] 6.3 Update any setup UI or state integration needed to call the native setup status and setup submission commands.

## 7. Tests

- [x] 7.1 Add application-service tests for successful setup, unsupported provider, empty key, invalid provider key, existing setup rejection, and concurrent setup behavior.
- [x] 7.2 Add repository tests for SQLite provider setup metadata persistence and incomplete local state detection.
- [x] 7.3 Add keyring abstraction tests using a fake secret store for read, write, presence, and rollback paths.
- [x] 7.4 Add RunPod provider tests using mocked GraphQL responses for valid active key, rejected key, inactive key, no matching key, provider timeout, and malformed response.
- [x] 7.5 Add command binding export test coverage for the new command contract.

## 8. Verification

- [x] 8.1 Run `cargo test`.
- [x] 8.2 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 8.3 Run `cargo fmt`.
- [x] 8.4 Run `bun run build`.
- [x] 8.5 Run `bun run lint --fix`.

## 9. Provider Setup Sync

- [x] 9.1 Update OpenSpec design and spec artifacts to separate local status reads from explicit provider sync.
- [x] 9.2 Add provider setup metadata upsert support for sync refreshes.
- [x] 9.3 Add `sync_gpu_cloud_provider_setup` to the application service and Tauri command contract.
- [x] 9.4 Move orphan-key recovery out of `get_gpu_cloud_provider_setup` and into sync.
- [x] 9.5 Add frontend access for explicit provider setup sync.
- [x] 9.6 Add focused tests and regenerate bindings.
- [x] 9.7 Rerun required verification commands.

## 10. Provider Setup Delete

- [x] 10.1 Update OpenSpec design and spec artifacts for local provider setup deletion.
- [x] 10.2 Add provider setup metadata delete support.
- [x] 10.3 Add `delete_gpu_cloud_provider_setup` to the application service and Tauri command contract.
- [x] 10.4 Add focused tests and regenerate bindings.
- [x] 10.5 Add frontend access for local provider setup deletion.
- [x] 10.6 Rerun required verification commands.
