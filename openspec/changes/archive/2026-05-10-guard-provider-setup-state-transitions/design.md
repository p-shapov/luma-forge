## Context

LumaForge treats React as a thin client and the Tauri native layer as the authority for durable state, provider access, and secure storage. GPU Cloud Provider Setup is native-owned state backed by the secure keyring. Workspace Setup depends on that state before persisting a Draft Workspace.

The current provider setup command path constructs a fresh `ProviderSetupService` per command call. `setup()` reads the keyring, awaits RunPod identity validation, then writes the key. That leaves a race window where two concurrent setup calls can both observe missing setup and both report success.

Workspace creation has a related prerequisite window. It reads the provider key before validation and persistence. If provider setup deletion interleaves after that read, Workspace creation can persist a Draft Workspace even though provider setup is no longer complete by the time durable Workspace state is written.

## Goals / Non-Goals

**Goals:**

- Serialize provider setup state transitions for the same GPU Cloud Provider.
- Preserve the existing command request/response contracts and UI-safe error codes.
- Ensure setup success is derived from durable keyring state after write.
- Ensure Workspace creation either persists while provider setup is still complete or rejects with the existing provider setup error.
- Keep the coordination mechanism local to native-owned state and compatible with future additional providers.

**Non-Goals:**

- Do not add new frontend APIs or generated command types.
- Do not add a hosted backend service.
- Do not add SQLite persistence for provider setup.
- Do not implement Workspace Provisioning orchestration in this change.
- Do not change the product rule that repeated setup is rejected rather than treated as key rotation.

## Decisions

### Add a provider-keyed native coordinator

Introduce a native coordination boundary for provider setup operations, keyed by `GpuCloudProviderId`. In v1 this can contain one RunPod gate, but the public shape should make the provider key explicit so the design scales when more providers are introduced.

The coordinator should be owned by the Tauri application state and injected into commands that mutate or depend on provider setup. This avoids relying on fresh per-command service instances for state coordination.

Alternative considered: keep the lock inside `ProviderSetupService`. That does not work cleanly with the current command construction because each command creates a new service instance, so the lock would not be shared unless command construction also changed.

Alternative considered: rely on keyring atomicity. The current `SecretStore` exposes `read_api_key` and `replace_api_key`, not an atomic compare-and-set operation. macOS keyring also should not be treated as the application-level concurrency contract for multi-step provider validation.

### Hold the provider setup gate across setup validation and write

`setup_gpu_cloud_provider` should acquire the provider gate before checking for an existing key and should hold it until the submitted key has validated, the keyring write has completed, and the returned setup status has been re-derived from stored state.

This intentionally serializes the remote validation call. The setup path is rare, user-initiated, and mutates global provider setup state. Correctness is more important than allowing two concurrent validations for a resource that can only accept one successful creation.

Alternative considered: validate first, then lock and re-read before write. That prevents double success but still allows a request to validate a submitted key after another request has completed setup, weakening the existing rule that repeated setup is rejected before provider validation.

### Hold the provider setup gate across delete

`delete_gpu_cloud_provider_setup` should acquire the same provider gate before reading keyring state and hold it through deletion. A delete requested while setup is in progress should evaluate after setup completes; a setup requested while delete is in progress should evaluate after deletion completes.

This keeps create/delete behavior deterministic and makes each operation observe the latest durable keyring state.

Alternative considered: gate only setup. That fixes the P1 double-success race but still leaves setup/delete interleavings ambiguous.

### Gate Workspace creation while validating provider setup and persisting the Draft Workspace

`create_workspace` should acquire the provider setup gate for the request provider before reading the provider key and should hold it until the Workspace Catalog insert and re-read completes or fails. This makes provider setup completeness stable across the prerequisite check and the durable Workspace mutation.

The existing SQLite `PRIMARY KEY` remains the atomic duplicate-Workspace boundary. The provider setup gate is not a replacement for database uniqueness; it protects the cross-resource invariant between keyring state and Workspace Catalog mutation.

Alternative considered: re-read provider setup immediately before inserting the Workspace. That narrows the race but does not prevent deletion from interleaving between the final read and the insert.

### Keep read-only status and inventory paths ungated unless a later requirement needs stronger snapshots

`get_gpu_cloud_provider_setup` and provider inventory lookup can remain read-only operations that observe current keyring state and call the provider. They do not mutate durable state. If they race with setup/delete, returning the latest observable state or an existing setup error is acceptable under the current contracts.

Workspace creation is different because it mutates durable Workspace Catalog state based on provider setup being complete.

## Risks / Trade-offs

- Holding a mutex across RunPod validation can block a concurrent delete request while the provider API is slow. Mitigation: setup is rare, request timeouts already exist in the provider client, and serializing the full mutation boundary preserves the strict repeated-setup contract.
- Holding a mutex across Workspace Catalog insertion can briefly block provider setup deletion. Mitigation: Workspace creation is local and short-lived after validation; this protects a more important consistency guarantee.
- A process-local coordinator cannot coordinate with a second app process using the same keyring. Mitigation: LumaForge is a single-user desktop app; this change addresses in-app command concurrency. Cross-process locking can be revisited if multi-instance execution becomes supported.
- New command state injection may affect binding export tests or command registration. Mitigation: keep request and response DTOs unchanged and update command wiring tests as part of implementation.

## Migration Plan

No user data migration is required. Provider setup remains stored only in the secure keyring, and Workspace Catalog schema remains unchanged.

Implementation can be rolled back by removing the coordinator state and restoring direct service construction, with no durable data conversion.

## Open Questions

- Should the coordinator live under `provider_setup`, `commands`, or a small native application state module? Default: keep it close to provider setup if it exposes provider-domain gates, but inject it at the command boundary.
- Should tests model lock ordering between provider setup and Workspace Catalog access explicitly? Default: yes, add targeted concurrent command/service tests so future provisioning work can follow the pattern.
