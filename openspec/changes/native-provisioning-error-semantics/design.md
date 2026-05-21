## Context

Workspace Provisioning is the orchestration boundary that decides whether a failed operation is safe to retry as a command, should remain visible as non-mutating progress, or must be persisted on the Workspace as recovery-required failure state. Workspace Resources is the lower resource-operation boundary that talks to provider capabilities, persists provider resource snapshots, and handles local Provisioner Worker token lifecycle around provisioning pods.

The current implementation already has several stable app-owned categories, including granular Workspace Catalog errors and provider operation uncertainty, but some paths collapse categories too early. The Phase 1 design is to document and harden the error boundary semantics before broad implementation work, with tests making the command-error versus persisted-failure split explicit.

## Goals / Non-Goals

**Goals:**

- Define the boundary between immediate command errors and persisted `WorkspaceProvisioningFailure` records.
- Preserve granular Workspace Catalog categories through Workspace Resources and Workspace Provisioning into command mapping.
- Clarify which categories belong at `WorkspaceResourceError` and which belong at `WorkspaceProvisioningError`.
- Keep provider resource corruption, missing resources, orphaned resources, cleanup failures, and indeterminate provider operations persisted on the Workspace when recovery is required.
- Keep frontend command errors stable, granular, retry-aware, and implementation-safe.
- Add regression tests for resource-to-provisioning mapping, provisioning-to-command mapping, persisted failure behavior, and token lifecycle behavior.

**Non-Goals:**

- Redesign the full provisioning workflow.
- Implement frontend UI changes.
- Expose raw SQLite, reqwest, keyring, RunPod, provider response, or secret details in frontend contracts.
- Refactor keyring access to async `spawn_blocking`.
- Add support for new providers.
- Replace all production `expect` calls.

## Decisions

1. Use `WorkspaceResourceError` as the provider resource operation boundary.

   `WorkspaceResourceError` will represent failures that occur while resource operations are reading or writing catalog state, reading or deleting secrets, calling provider APIs, observing provider resources, handling provider operation uncertainty, or managing Provisioner Worker bearer tokens.

   Alternative considered: return provider-local errors directly from resource operations and map them in Workspace Provisioning. That would blur provider boundaries and force orchestration code to understand provider adapter details.

2. Use `WorkspaceProvisioningError` as the orchestration and command failure boundary.

   `WorkspaceProvisioningError` will represent failures that should escape a provisioning command immediately: invalid workspace identity or lifecycle, catalog/persistence failure, secret/keyring failure, transient provider or worker availability failures, conflicts, and resource-operation failures that do not require persisted Workspace recovery state.

   Alternative considered: persist every provisioning error as a Workspace failure. That would make transient storage, keyring, conflict, and rate-limit problems look like durable Workspace corruption and would create unnecessary recovery work.

3. Persist Workspace recovery failures when provider/resource state may be unsafe or requires cleanup.

   Provider operation indeterminate, tracked provider resource missing, orphaned provider resources, cleanup failure, terminal worker failure, worker API contract failure, and token inconsistency during environment preparation will remain represented as structured `WorkspaceProvisioningFailure` metadata when the Workspace needs recovery. These failures should not be hidden behind generic command errors.

   Alternative considered: return all provider/resource failures as command errors and leave Workspace state unchanged. That loses the durable recovery trail needed when remote provider state may exist or cleanup is required.

4. Preserve catalog categories as command errors without mutating Workspace state.

   SQLite storage/query/migration failures and catalog corruption/schema mismatch are local persistence problems. Provisioning/resource layers will preserve those categories and return granular command errors without creating, deleting, or modifying provider resources and without persisting new Workspace failure state.

   Alternative considered: persist catalog failures on the Workspace. That is not reliable because the catalog itself is the failing persistence boundary.

5. Keep command mapping app-owned and implementation-safe.

   `NativeCommandError` mapping will use stable LumaForge-owned codes, reasons, retryability, and recovery actions. Low-level SQLite, reqwest, keyring, RunPod, HTTP, GraphQL, raw response, provider-specific error, filesystem details, Provider API Key, and Provisioner Worker token details stay below the command boundary.

   Alternative considered: include low-level details in command errors for easier debugging. That would make frontend contracts unstable and risks exposing implementation details or secrets.

6. Treat Provisioner Worker token lifecycle by phase and provider certainty.

   If token creation/write succeeds and provisioning pod creation then fails with a determinate no-pod-created result, the resource layer will attempt best-effort local token deletion while preserving the original failure category. If a pod may exist or provider state is indeterminate, token deletion must not erase the credential needed for inspection or recovery. During environment preparation, token missing or invalid errors indicate native state inconsistency and should be persisted as Workspace failure metadata.

## Decision Matrix

| Situation | Expected behavior |
| --- | --- |
| SQLite query/storage/migration failure | Return granular command error; do not mutate workspace state. |
| Catalog corrupt/schema mismatch | Return granular command error; do not mutate workspace state. |
| Provider API unavailable/rate limited | Return retryable command error or non-mutating progress; do not persist failure unless workflow semantics require it. |
| Provider request rejected/response invalid | Return command error or persist failure according to phase-specific semantics. |
| Provider operation indeterminate | Persist workspace failure with cleanup recovery action when resource state may be unsafe. |
| Tracked provider resource missing | Persist workspace failure with cleanup recovery action. |
| Orphaned provider resources discovered | Persist workspace failure with cleanup recovery action and stable UI-safe failure code/source metadata. |
| Provisioner worker token missing/invalid during environment prep | Persist workspace failure as native state inconsistency. |
| Provisioner worker temporarily unreachable | Keep as running/readiness progress; do not persist failure. |
| Provisioner worker unauthorized/invalid response/terminal failure | Persist workspace failure with inspect/recovery action. |
| Cancel cleanup failure | Persist workspace failure with cleanup recovery action. |

## Risks / Trade-offs

- Error categories can multiply across boundaries -> Keep mappings explicit and regression-tested from resource to provisioning to command.
- Phase-specific behavior may be misapplied -> Encode the decision matrix in tests for representative provisioning phases.
- Persisting recovery failures requires a working catalog -> If catalog persistence fails while recording failure state, return the granular catalog command error and preserve provider metadata already available in memory.
- Best-effort token cleanup can fail -> Preserve the original pod creation category and keep secrets out of returned errors, persisted failure metadata, and logs.
- Command codes may change accidentally -> Test command code, reason, retryability, and recovery action for each supported category.

## Migration Plan

This change is an internal native-layer semantics hardening with no planned database schema migration. Existing persisted Workspace records remain compatible. If generated command bindings change because new error codes are exported, regenerate them as part of implementation and keep the frontend-facing shape stable.

## Open Questions

- Exact new `NativeCommandErrorCode` discriminant names can be selected during implementation, but each must be stable, UI-safe, and covered by mapping tests.
