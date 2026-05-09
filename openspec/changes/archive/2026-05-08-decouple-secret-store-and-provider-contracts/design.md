## Context

Provider Setup currently owns `ProviderSetupError` and the generated command DTO `GpuCloudProviderId`. Both have become shared indirectly:

- `SecretStore` returns `ProviderSetupError`, so Workspace Setup must convert Provider Setup errors even though it only needs secure keyring access.
- Workspace contracts and workspace catalog persistence import `GpuCloudProviderId` from `provider_setup`, making Provider Setup act as a shared native contract module.

The current user-visible behavior is acceptable. The issue is dependency direction and ownership: shared infrastructure and shared command contracts should not depend on one application flow.

## Goals / Non-Goals

**Goals:**

- Make secret storage use-case independent by introducing a `SecretStoreError`.
- Preserve existing setup and workspace error codes, messages, and retryability semantics.
- Move shared provider command DTOs into a neutral native contract module.
- Keep generated TypeScript command shapes compatible with the current frontend contract.
- Keep the change scoped to the P2 boundary issues.

**Non-Goals:**

- Do not perform the broader refactor where Tauri command DTOs are mapped into separate application request/response types before entering services.
- Do not change the domain `GpuCloudProviderId` into a generated command DTO.
- Do not change provider setup, workspace setup, keyring, or persistence behavior.
- Do not add new provider ids or persistence migrations.

## Decisions

### Secret storage owns secret-specific failures

Add `SecretStoreError` in `secrets.rs` and make the `SecretStore` trait return it from read, replace, and delete operations.

Expected variants:

- `SecureKeyringUnavailable`
- `InvalidStoredProviderApiKey`

`SecureKeyringUnavailable` covers keyring entry creation, read, write, and delete failures except a missing entry. `InvalidStoredProviderApiKey` covers a keyring value that exists but cannot be parsed as a valid `ProviderApiKey`.

Provider Setup and Workspace Setup will each implement `From<SecretStoreError>` for their own use-case error type. `InvalidStoredProviderApiKey` will preserve current behavior by mapping to `InvalidProviderApiKey`.

Alternative considered: keep using `ProviderSetupError` and only adjust workspace mappings. This leaves the infrastructure trait coupled to one use case and does not resolve the review issue.

Alternative considered: map invalid stored keys to `SecureKeyringUnavailable`. This would reduce user-facing specificity and change current behavior.

### Shared provider command DTOs live in a neutral contract module

Move the generated command DTO `GpuCloudProviderId` and its domain conversions from `provider_setup_contracts.rs` into a neutral shared contract module under `src-tauri/src`, for example:

```text
src-tauri/src/shared_contracts/
  mod.rs
  provider_contracts.rs
```

Provider Setup, Workspace Setup contracts, Workspace Catalog persistence, and tests should import the DTO from this shared module. Provider Setup may re-export the DTO only if needed for compatibility inside provider setup tests, but other modules must not depend on Provider Setup as the owner of shared DTOs.

Alternative considered: place shared DTOs under `commands/shared_contracts/provider_contracts`. This matches the command-facing nature of the DTO, but current application services directly consume command DTOs. Importing from `commands` would make service modules depend on the Tauri command boundary, which is worse for the current architecture.

Alternative considered: use the domain `GpuCloudProviderId` everywhere and derive `serde`/`specta` on it. This would leak generated binding concerns into the domain model, contradicting the existing native boundary requirements.

### Broader command/application contract split is deferred

The cleaner long-term shape is for command handlers to own generated DTOs and map into application-layer inputs before invoking services. That would remove command DTOs from services entirely.

This change intentionally does not perform that refactor because the P2s can be fixed with less churn and without changing service APIs broadly.

## Risks / Trade-offs

- Shared contract module can become a dumping ground. Mitigation: add only provider command DTOs that are genuinely shared by multiple native flows.
- Re-exporting `GpuCloudProviderId` from Provider Setup could hide remaining wrong imports. Mitigation: update workspace and persistence imports to use the neutral shared module directly.
- Generated TypeScript output could reorder type declarations. Mitigation: verify generated bindings export the same `GpuCloudProviderId = "runpod"` shape and run native tests.
- `InvalidStoredProviderApiKey` may appear rare but security-sensitive. Mitigation: keep errors UI-safe and do not expose stored secret values in messages, diagnostics, or generated types.
