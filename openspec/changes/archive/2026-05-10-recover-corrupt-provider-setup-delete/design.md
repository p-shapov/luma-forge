## Context

Provider setup is derived from a Provider API Key stored in the macOS secure keyring. Most flows need a parsed, domain-valid `ProviderApiKey`: setup status validates identity, workspace setup checks provider readiness, and provider clients need a usable secret.

Deletion is different. Deleting local setup only needs to know whether the provider-owned keyring entry exists, then remove that entry. The current implementation uses the parsed `read_api_key()` path as a presence check, so corrupt or blank stored key material prevents deletion before `delete_api_key()` is reached.

The current flow documentation says invalid local provider/key state is recoverable through Factory Reset, but Factory Reset is not implemented. A provider-specific delete operation is the smallest recovery path for this state.

## Goals / Non-Goals

**Goals:**

- Allow `delete_gpu_cloud_provider_setup` to delete a present local provider keyring entry even when its stored value cannot be parsed as a valid Provider API Key.
- Keep missing-entry deletion non-idempotent by returning `provider_setup_incomplete`.
- Keep parsed reads strict for status, setup creation, workspace setup, and provider client usage.
- Avoid exposing Provider API Key values to React, logs, generated types, or diagnostics.

**Non-Goals:**

- Implement Factory Reset.
- Revoke API keys in RunPod.
- Add key rotation or replacement behavior.
- Change command request or response DTOs.
- Treat corrupt stored key material as a complete provider setup.

## Decisions

### Add raw keyring-entry presence to the secret store boundary

Add a secret-store operation with semantics like `has_api_key_entry(provider_id) -> Result<bool, SecretStoreError>`. The method checks whether the provider keyring entry exists without parsing the stored value as `ProviderApiKey`.

`delete_setup()` should use this raw entry check before deletion:

```text
delete_setup(provider)
  ├─ has_api_key_entry(provider) == false -> provider_setup_incomplete
  ├─ has_api_key_entry(provider) == true  -> delete_api_key(provider)
  └─ keyring access failure               -> secure_keyring_unavailable
```

The name should include `entry` to avoid implying that a domain-valid Provider API Key exists. A corrupt entry is a recoverable local setup artifact, not a usable setup.

Alternative considered: make `delete_api_key()` return a deletion outcome such as `Deleted | Missing`. That is also a clean boundary and avoids a separate presence check, but it changes the existing delete contract more broadly. A dedicated entry-presence method keeps this change narrow and matches the current service shape.

Alternative considered: keep relying on `read_api_key()` and add special handling for `InvalidStoredProviderApiKey`. That would work, but it keeps delete coupled to parsing and makes the service infer raw keyring state from a domain parsing failure.

### Keep strict parsed reads for usable setup state

`read_api_key()` should continue to parse the stored value and return `InvalidStoredProviderApiKey` when the entry cannot become a `ProviderApiKey`. Status reads, setup existence checks for create, workspace setup, and provider clients should keep using this strict path because those flows need a usable secret.

This preserves fail-closed behavior: corrupt stored state is not reported as complete setup and is not used for provider calls.

Alternative considered: loosen `ProviderApiKey::new()` for stored values. This would hide corrupt local state and push invalid secrets deeper into provider calls, producing less precise behavior.

### Preserve command contract shape

The delete command should keep returning `gpu_cloud_provider_setup: null` on success and `provider_setup_incomplete` when no local keyring entry exists. The corrupt-entry case becomes successful deletion because the setup artifact exists locally and the requested recovery action completed.

No new frontend type is needed. UI behavior can reuse the existing successful-delete path.

## Risks / Trade-offs

- Raw entry presence may briefly race with external keychain mutation outside the app -> keep current error handling on `delete_api_key()` and map keyring failures to `secure_keyring_unavailable`.
- A present but unreadable keychain entry may still fail the raw presence check on some keyring backend errors -> treat backend access failures as `secure_keyring_unavailable`, not as missing setup.
- The additional secret-store method increases interface surface -> keep it narrow, provider-scoped, and documented as raw presence only.
- Setup creation currently rejects an existing corrupt entry through the strict read path as `invalid_provider_api_key` rather than `provider_setup_already_exists` -> leave that unchanged unless a later proposal defines corrupt setup state explicitly.
