## Context

The GPU Cloud Provider Setup flow defines the first durable provider-owned state that later Workspace Setup and Workspace Provisioning flows depend on. The current native layer exposes only placeholder commands, so this change establishes the native architecture pattern for command adapters, application services, domain types, SQLite repositories, keyring access, and provider implementations.

The flow has a hard secret-handling boundary: React submits the Provider API Key once, Native Layer validates and stores it, and no response, log, generated binding, domain snapshot, or diagnostic message may contain the secret. v1 supports RunPod only, but the native code should isolate RunPod-specific GraphQL mapping behind a small GPU Provider interface so future provider support does not leak into application services.

## Goals / Non-Goals

**Goals:**

- Implement native-owned setup status, setup sync, setup deletion, and setup submission commands with generated `specta` / `tauri-specta` bindings.
- Store mutable provider setup metadata in SQLite and Provider API Keys in the secure keyring.
- Validate RunPod API keys through a provider abstraction using an identity-focused provider call.
- Return redacted provider setup status with provider id, provider user id, and provider API key fingerprint.
- Reject repeated setup once complete setup exists.
- Keep normal status reads local-only and side-effect-free.
- Support explicit sync when keyring contains a provider key by validating the existing key and refreshing or restoring redacted metadata.
- Support explicit local deletion of a provider setup by removing the Provider API Key from keyring and setup metadata from SQLite.
- Preserve fail-closed mutation ordering and rollback behavior for keyring/config persistence failures.

**Non-Goals:**

- Do not support GPU Cloud Providers other than RunPod in v1.
- Do not validate provider permissions during setup beyond proving provider identity and matching an active provider key record.
- Do not implement Workspace Setup, Workspace Provisioning, Factory Reset, or Workspace Resource Cleanup.
- Do not introduce a hosted backend service.
- Do not run ML inference locally.
- Do not expose RunPod-specific API response shapes to React.

## Decisions

### Use a small GPU Provider interface

Application services will depend on a `GpuProvider` port instead of calling RunPod directly. The first operation will be key validation:

```text
GpuProvider::validate_api_key(secret) -> ValidatedProviderCredential
```

`ValidatedProviderCredential` contains only UI-safe metadata needed by the setup flow: provider user id and provider API key fingerprint.

Alternatives considered:

- Calling RunPod directly from the setup service would be faster to implement, but it would mix provider-specific GraphQL behavior with flow ordering and rollback logic.
- Designing a broad provider SDK upfront would overfit unimplemented provisioning behavior. The interface should grow by workflow.

### Keep keyring access outside provider implementations

Provider implementations receive a secret when they need to call the provider, but they do not read or write keyring entries themselves. The application service owns when secrets are read, written, deleted, and zeroized.

Alternatives considered:

- Letting `RunPodProvider` own keyring access would hide secret storage details, but it would make setup rollback and orphan-key recovery harder to reason about.

### Persist setup metadata in SQLite

SQLite stores mutable user data for provider setup. The setup record includes provider id, provider user id, provider API key fingerprint, and timestamps. The Provider API Key itself is stored only in the secure keyring.

Normal `get_gpu_cloud_provider_setup` reads SQLite metadata and checks keyring presence without calling the provider. `sync_gpu_cloud_provider_setup` reads the existing Provider API Key from keyring, validates it with the provider, and persists refreshed metadata only after validation succeeds.

Alternatives considered:

- A JSON config file would be simpler for one record, but the project direction is that mutable user data belongs in SQLite.
- Revalidating with RunPod on every status read would keep metadata fresh, but it would make setup status network-dependent and noisy. A separate sync command keeps that behavior explicit.

### Validate RunPod identity and active key identity only

`RunPodProvider::validate_api_key` calls RunPod GraphQL `myself` and derives:

- `provider_user_id` from `myself.id`
- `provider_api_key_fingerprint` from the active `apiKeys[].id` that matches the submitted key prefix

The provider implementation observes permissions but does not enforce them during setup. Later flows fail at their operation boundary if permissions are insufficient.

Alternatives considered:

- Enforcing RunPod permissions during setup could catch more errors early, but permission requirements are flow-specific and may change as provisioning behavior is implemented.
- Using a local hash as the fingerprint would be provider-independent, but RunPod exposes a stable redacted API key id that is more recognizable to users and support diagnostics.

### Fail closed with strict mutation ordering

Setup submission performs mutations in this order:

1. Validate request and existing setup state.
2. Validate Provider API Key with the selected provider.
3. Store Provider API Key in keyring.
4. Persist provider setup metadata in SQLite.
5. Re-read local setup state and return redacted status.

If SQLite persistence fails after keyring write succeeds, Native Layer attempts to delete the newly written key and rejects setup. If rollback deletion fails, Native Layer still rejects setup and must not report completed setup.

### Separate local status from provider sync

`get_gpu_cloud_provider_setup` has no provider side effects and no SQLite writes. It returns complete setup only when local metadata and keyring presence are consistent.

`sync_gpu_cloud_provider_setup` performs provider-backed refresh and recovery. It returns incomplete setup when no key exists, and it rejects invalid keys without deleting local metadata or keyring secrets; destructive recovery remains outside this change.

`delete_gpu_cloud_provider_setup` is local-only. It deletes the selected provider keyring entry first, then deletes matching SQLite setup metadata. It does not call the Provider and does not delete Provider Resources.

### Serialize provider setup mutation and sync

The native layer will guard setup sync and setup submission with a process-local async lock. This prevents two concurrent setup requests from both passing the "no complete setup exists" check before either persists metadata, and it keeps sync refreshes from racing setup writes.

SQLite uniqueness on provider setup metadata should also prevent duplicate complete rows.

## Risks / Trade-offs

- RunPod `ApiKey.id` matching could change or fail for legacy keys -> Treat unmatched active keys as invalid for setup and surface a provider validation error; keep the matching logic isolated in `RunPodProvider`.
- Explicit sync can fail when RunPod is unavailable -> Keep `get_gpu_cloud_provider_setup` local-only so the UI can still display last-known complete setup while offline.
- Delete can leave incomplete local state if SQLite deletion fails after key deletion -> Report the local storage error and rely on the status command to fail closed as incomplete setup until local recovery or retry succeeds.
- Keyring rollback after SQLite failure is best effort -> Reject setup regardless of rollback outcome and do not treat orphaned keys as complete setup unless recovery later validates and persists metadata.
- SQLite schema becomes the first native persistence boundary -> Keep migrations focused and covered by repository/service tests.
- Provider permissions are not checked during setup -> Later workspace flows must map insufficient-permission provider errors to user-visible errors and `Failed` workspace state when provisioning cannot continue.

## Migration Plan

This is the first implementation of provider setup state, so no existing production data migration is required. Add SQLite migrations for provider setup metadata and initialize the native database before commands can use provider setup services.

Rollback during development can remove the OpenSpec change and implementation files. Runtime rollback after a partial setup attempt is handled by the setup service: keyring writes are rolled back if SQLite metadata persistence fails, and incomplete local state is not reported as complete setup.

## Open Questions

None.
