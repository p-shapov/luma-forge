## Context

LumaForge is a macOS-only Tauri application where React owns presentation and the Rust native layer owns provider access, durable state, secure storage, and authoritative workflow decisions. The current Rust command boundary is still scaffold-level and only exposes demo commands.

The GPU Cloud Provider Setup flow establishes whether LumaForge has a usable RunPod API key before later workspace setup and provisioning flows attempt provider calls. In v1, RunPod is the only supported GPU Cloud Provider. The native layer must never return Provider API Keys to React and must not store them in SQLite, app config, generated bindings, logs, or diagnostics.

The settled flow differs from the earlier flow document in two important ways:

- Setup state is derived live from the secure keyring and RunPod identity, not from a separately persisted selected-provider config.
- Deletion removes only the local keyring entry and does not revoke or delete the API key in RunPod.

## Goals / Non-Goals

**Goals:**

- Add native commands for reading, setting once, and deleting GPU Cloud Provider setup.
- Keep Provider API Keys native-only and store them only in the secure keyring.
- Validate submitted keys using only a RunPod identity check.
- Return a redacted `GpuCloudProviderSetup` projection that includes provider id, provider user email, and provider API key fingerprint.
- Derive the RunPod fingerprint from the matched RunPod `apiKeys[].id` value.
- Reject setup submission before provider validation or keyring mutation when a complete setup already exists.
- Keep command handlers thin and put workflow decisions in an application service.

**Non-Goals:**

- No full React setup UI implementation.
- No Provider API permission probing during setup.
- No local SQLite or filesystem persistence for provider setup state.
- No RunPod API key revocation.
- No support for providers other than RunPod in v1.
- No workspace setup or provider resource provisioning behavior.

## Decisions

### Use Live Status Instead of Persisted Local Provider Setup

`get_gpu_cloud_provider_setup` reads the provider-specific keyring entry. If no key exists, it returns `gpu_cloud_provider_setup: null`. If a key exists, it calls the provider identity capability and returns the derived setup snapshot.

This avoids a second local state source that can drift from keyring state. The tradeoff is intentional: setup status requires network access whenever a key exists.

Alternative considered: persist selected provider and redacted setup metadata locally. This was rejected for this slice because v1 supports only RunPod and the user prefers deriving setup validity directly from the stored key plus provider identity.

### Treat Setup Submit as One-Time Creation

`setup_gpu_cloud_provider` first checks whether a keyring entry already exists for the requested provider. If a stored key exists, the native layer returns `provider_setup_already_exists` before validating the submitted key or mutating the keyring.

If no key exists, the native layer validates the submitted key with RunPod before writing it, then re-reads the stored key and returns live setup status for the key that was actually stored.

Alternative considered: allow repeated setup as key replacement. This was rejected to preserve the canonical flow contract that repeated setup after a complete setup is not a key-rotation path.

### Use Closed Provider IDs at the Command Boundary

Command request types use a closed `GpuCloudProviderId` enum. In v1 this enum contains only `runpod`, so unsupported provider ids are treated as command payload/schema violations and can fail during Tauri argument deserialization before the provider setup service runs.

This keeps the generated TypeScript contract strict. The tradeoff is intentional: these commands do not guarantee a structured `unsupported_provider` domain error for arbitrary raw invoke payloads. If LumaForge later needs structured unsupported-provider errors at the native command boundary, the request field should change back to a raw string or custom-deserialized value that the command handler can parse explicitly.

### Add Explicit Local Delete

`delete_gpu_cloud_provider_setup` requires a supported provider id, reads the keyring, and deletes the local keyring entry only if setup exists. If the key is already missing, it returns `provider_setup_incomplete`.

Deletion does not call RunPod and does not revoke the provider-side API key. This keeps local setup deletion separate from provider account management.

Alternative considered: make delete idempotent and return success when the key is already missing. This was rejected because a delete request should target an existing configured provider setup.

### Use Capability-Based Provider Abstractions

Introduce a narrow provider identity capability instead of a broad generic provider client. The setup service depends on a provider registry that resolves the supported provider id to an identity-capable provider client.

The initial shape should stay close to the current needs:

```text
Commands
  -> ProviderSetupService
       -> SecretStore
       -> ProviderRegistry
            -> ProviderIdentityClient
                 -> RunPodClient
```

Future flows can add separate capabilities for inventory, resource provisioning, and cleanup without forcing setup to depend on those APIs.

Alternative considered: define one large `GpuCloudProviderClient` abstraction for all future behavior. This was rejected because it would either overfit RunPod or introduce unused abstraction before the other flows need it.

### Derive Fingerprint From RunPod API Key Prefix

RunPod identity validation uses a GraphQL `myself` query that returns the provider user email and `apiKeys` entries. The RunPod adapter matches the submitted or stored secret against `apiKeys[].id` using a prefix rule:

```text
exactly one apiKeys item where secret.starts_with(apiKey.id)
```

The matched API key must have `isActive == true`. The external field remains `provider_api_key_fingerprint`; internally this value is a RunPod API key id.

If authentication succeeds but identity or fingerprint derivation fails, the service returns `provider_identity_unavailable`.

Alternative considered: derive a local hash from the secret. This was rejected because RunPod already exposes a redacted API key identifier suitable for status and diagnostics.

### Error Mapping

All keyring read, write, and delete failures map to `secure_keyring_unavailable`.

Provider authentication failures, inactive matched API keys, and empty submitted keys map to `invalid_provider_api_key`.

Network timeouts, transport failures, and provider unavailability map to `provider_api_unavailable`.

Valid provider authentication with unexpected or ambiguous identity/key matching maps to `provider_identity_unavailable`.

`local_storage_unavailable` is reserved for future SQLite or filesystem-backed state and is not used by this provider setup slice.

## Risks / Trade-offs

- Live status depends on network access -> status can fail while offline even if a key is stored. This is acceptable because status represents current provider-valid setup, not local key presence.
- RunPod fingerprint matching depends on key-format behavior -> keep the prefix-matching rule isolated inside the RunPod adapter and return `provider_identity_unavailable` if it becomes ambiguous or unsupported.
- Setup is not a key-rotation mechanism -> users need an explicit delete-then-setup path until a dedicated key-rotation flow exists.
- Provider identity request may expose more fields than needed -> request only `myself.email` and `apiKeys { id isActive }`, and do not log secrets or full provider responses containing sensitive data.
- Future providers may not expose equivalent API key identifiers -> keep `provider_api_key_fingerprint` as a provider-specific redacted value and add adapter-specific derivation later.

## Migration Plan

No user data migration is required because provider setup state does not currently exist in native storage.

Implementation can replace the demo commands with real generated command bindings for this capability. If a later local persistence layer is introduced, it must not backfill or store Provider API Keys.

## Open Questions

None.
