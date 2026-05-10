## Context

LumaForge exposes native operations through generated Tauri command bindings. The command boundary already returns UI-safe `NativeCommandError` values containing `code`, `message`, and `retryable`, and existing native-boundary rules require these errors to avoid provider secrets and provider transport details.

The current code maps many distinct causes into broad errors too early. Examples include `invalid_request` for malformed Workspace creation input, `invalid_placement_plan` for several different stale or incomplete placement states, `workflow_catalog_unavailable` for multiple catalog/profile readers, `provider_api_unavailable` for network, HTTP, GraphQL, and malformed-response failures, and `workspace_catalog_unavailable` for storage, migration, query, corruption, and row-consistency failures.

The goal is not to expose internal implementation details to React. The goal is to preserve the secure command boundary while making failures precise enough for users, tests, diagnostics, and recovery actions.

## Goals / Non-Goals

**Goals:**

- Add more specific UI-safe command error codes for the existing Provider Setup and Workspace Setup command surface.
- Keep command error DTOs owned by the command boundary and generated through `specta` / `tauri-specta`.
- Preserve secret isolation: no Provider API Key, keyring value, raw provider response body, raw SQL error, raw persisted Workspace JSON, or implementation stack detail may cross into React.
- Add optional safe error metadata for field-level or recovery-oriented UI behavior.
- Improve frontend command-console error rendering so users see actionable copy instead of only raw JSON.
- Keep read-command failures and mutation failures distinguishable where recovery differs.

**Non-Goals:**

- Do not add provisioning command errors in this change unless those commands are implemented in the same branch.
- Do not expose raw source errors or provider response payloads to command responses.
- Do not change Workspace Catalog persistence format.
- Do not add a hosted backend or remote error-reporting service.
- Do not change successful command response payload shapes except regenerated bindings reflecting the error contract.

## Decisions

1. Expand `NativeCommandErrorCode` rather than overloading `message`.

   Error code is the stable contract React can switch on. Human-readable `message` remains UI-safe fallback text and must not be parsed.

   Alternative considered: keep the current codes and only improve messages. That would help the command console but would not give React reliable recovery logic, field highlighting, or tests.

2. Add optional safe metadata to `NativeCommandError`.

   Extend the command error DTO with optional fields such as `field`, `reason`, and `recovery_action` or an equivalent structured shape. Metadata must be enum/string-literal based and UI-safe. It must not include raw source errors.

   Alternative considered: create a separate error DTO per command. That gives maximum precision but fragments frontend handling and complicates generated bindings for the current small command surface.

3. Use typed internal errors where low-level causes matter, then map once at the boundary.

   Provider clients, secret storage, bundled catalog reading, Workspace Catalog persistence, and use-case services should preserve non-secret categories internally until command mapping. Mapping directly with `map_err(|_| BroadError)` at the failure site should be reserved for cases where the lost detail is intentionally irrelevant.

   Alternative considered: map every failure directly to command errors in services. That would leak command concerns into application services and conflict with native-boundary ownership.

4. Keep provider and persistence details abstract at the command boundary.

   Codes may distinguish `provider_network_unavailable`, `provider_response_invalid`, or `workspace_catalog_corrupt`, but command responses must not expose HTTP status bodies, GraphQL payloads, SQLite messages, file paths beyond a safe category, or raw Workspace JSON.

   Alternative considered: include source error strings in debug builds. That risks accidental leakage through screenshots, logs, or generated payloads and should be handled separately by safe diagnostics/tracing.

5. Treat frontend presentation as part of the command error contract.

   React should render command errors through a shared presenter keyed by `NativeCommandErrorCode`, showing concise copy and recovery actions such as retry, refresh provider setup, reload catalogs, refresh Workspace Catalog, or reselect placement data. The command log may still show raw JSON for development, but it should not be the only user-facing explanation.

   Alternative considered: leave presentation to future product UI. The current native command console is the active surface for these flows, and clearer errors are needed now to debug and validate behavior.

## Risks / Trade-offs

- More error codes increase contract surface area -> keep codes grouped by command domain and cover them with mapping tests.
- Some codes may need refinement when full product UI replaces the command console -> use stable high-level categories and optional metadata rather than overly specific implementation names.
- Mapping all Workspace Catalog corruption variants separately may overlap with the narrower `refine-workspace-catalog-error-handling` change -> coordinate by letting that change own internal repository errors while this change owns command-facing codes and presentation.
- Provider APIs may return ambiguous failures -> classify conservatively as unavailable or invalid response when no safe, actionable distinction exists.
- Generated binding changes require frontend updates -> regenerate and commit bindings in the same implementation change.

## Migration Plan

1. Add or refine internal error categories in provider, secret store, bundled catalog, workspace setup validation, and workspace catalog boundaries.
2. Expand `NativeCommandErrorCode` and `NativeCommandError` with optional UI-safe metadata.
3. Update command error mapping tests before changing service mappings.
4. Update services and validators to preserve and map precise non-secret categories.
5. Regenerate TypeScript command bindings and update spec reference native contracts.
6. Add a frontend native-command error presenter and use it in the command console.
7. Run required frontend and backend verification.

## Open Questions

- Should `field` be a single optional string literal or a list when multiple request fields are invalid?
- Should `recovery_action` be a command-neutral enum, command-specific enum, or only frontend-local mapping from code to action?
- Should Workspace Catalog corruption/future-version cases remain under `workspace_catalog_unavailable` until the existing narrower Workspace Catalog error proposal is implemented, or should this change introduce command-level variants immediately?
