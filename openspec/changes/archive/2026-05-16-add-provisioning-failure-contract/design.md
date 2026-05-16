## Context

Workspace Provisioning already has three related concepts that are not clearly separated:

- `WorkspaceLifecycleState::Failed` records that a workspace cannot continue provisioning from its current durable state.
- `WorkspaceProvisioningError` maps provider, worker, keyring, and catalog failures to UI-safe `NativeCommandError` responses.
- `WorkspaceProvisioningProgress.message` was an unstructured optional string that could become an accidental error channel, so the progress contract removes it instead of deprecating it.

The provisioning service currently marks workspaces failed for terminal provider resource observations, terminal worker failures, missing worker tokens, cancellation cleanup failure, and some indeterminate creation outcomes. Other provider failures bubble as `NativeCommandError` without a durable lifecycle mutation. That separation is useful, but the contract does not explicitly tell implementers or React which channel is authoritative for each failure class.

## Goals / Non-Goals

**Goals:**

- Introduce a typed, UI-safe provisioning failure detail that can be persisted with Workspace metadata and returned in generated bindings.
- Make `Failed` workspace state explainable after the command that caused the failure has completed.
- Preserve `NativeCommandError` for sync attempts that fail without changing durable workspace truth.
- Define provider error lifecycle rules around retryable transport/control-plane errors, provider request rejection, terminal resource observations, and unsafe continuation.
- Keep all secrets, raw provider responses, worker tokens, stack traces, and command output out of persisted metadata and command responses.

**Non-Goals:**

- Do not implement automatic repair or retry for failed workspaces.
- Do not add Workspace Resource Cleanup beyond preserving metadata for existing cleanup paths.
- Do not add provider-specific error payloads to React contracts.
- Do not change Provider Setup, Workspace Setup, or Provisioner Worker HTTP API behavior except where their existing UI-safe failures are represented in provisioning failure detail.

## Decisions

### Persist a domain-level provisioning failure detail

Add a Native-owned failure detail to Workspace metadata: `last_provisioning_failure: Option<WorkspaceProvisioningFailure>`. The type contains stable UI-safe fields:

- failure code
- failed phase
- source: native, provider, provider_resource, or provisioner_worker
- retryable flag
- recovery action
- optional sanitized diagnostic

Rationale: a failed workspace may be read long after the failing command response is gone. The workspace needs enough durable context for React to render the failed state and route the user toward cleanup, provider setup recovery, placement reselection, or retry.

Alternative considered: reuse `NativeCommandError` inside the domain model. Rejected because command errors are a boundary contract with fields such as `field` and command-specific recovery text. Persisting them would couple the domain layer to Tauri command response shape.

Alternative considered: keep using `WorkspaceProvisioningProgress.message`. Rejected because a string does not encode retryability, recovery, phase, source, or stable testable failure classes.

### Keep command errors for non-authoritative sync failures

Provider API failures that do not produce new durable truth should return `NativeCommandError` and leave existing Workspace metadata unchanged, except for any already-persisted authoritative observation. This includes provider rate limiting, provider API unavailability, operation conflict, and provider request rejection when Native can safely retry or let the user adjust input without losing cleanup metadata.

Rationale: a failed sync attempt is not always a failed workspace. Marking a workspace failed on every transient provider outage would turn recoverable control-plane failures into cleanup-required terminal states.

Alternative considered: mark the workspace failed for every provider API error after provisioning starts. Rejected because it would make ordinary rate limits and outages destructive to the lifecycle.

### Persist `Failed` only from terminal truth or unsafe continuation

The service should set `WorkspaceLifecycleState::Failed` when Native has learned durable terminal truth or cannot safely continue without risking duplicate resources, leaked resources, or a false `Ready` state. Examples:

- provider resource observation reports failed, terminated unexpectedly, unknown, or missing in a phase where safe continuation is impossible
- worker reports terminal failure or an unrecoverable worker API error
- readiness validation cannot confirm required resources
- cancellation cleanup cannot confirm deletion of known resources
- provider mutation outcome is indeterminate and Native cannot identify exactly one safe correlated resource

When this happens, the service should persist both the `failed` lifecycle and structured failure detail in the same Workspace Catalog update whenever possible.

Alternative considered: return `NativeCommandError` for terminal resource observations. Rejected because terminal observations are authoritative workspace state, not only command failure.

### Derive failed progress from persisted failure detail

`WorkspaceProvisioningProgress` should remain rendering and sync-loop state derived from Workspace metadata. For failed workspaces, progress should expose the persisted failure detail or a generic UI-safe fallback when older metadata lacks it.

Rationale: React already treats Native responses as authoritative. Deriving progress avoids duplicating error state between transient command responses and durable workspace records.

Alternative considered: persist progress snapshots. Rejected because existing provisioning specs treat worker progress as non-authoritative and derived.

### Preserve compatibility through explicit migration fallback

Existing workspace rows may lack structured failure detail. Reading them should continue to work. If a failed workspace lacks failure detail, Native should return failed progress with a generic failure classification and no unsafe diagnostic.

Rationale: the project already uses SQLite-backed Workspace Catalog persistence, so command and UI behavior must tolerate older saved workspaces.

## Risks / Trade-offs

- Contract churn for generated frontend bindings -> Keep the new type small, stable, and UI-safe; regenerate bindings and update frontend consumers in the same implementation change.
- Over-classifying provider errors -> Start with existing `WorkspaceProvisioningError` classes and map only stable categories, not provider-specific raw codes.
- Persisted diagnostics leaking sensitive data -> Store only sanitized diagnostics produced by trusted mapping code; never persist raw response bodies, tokens, API keys, stack traces, environment dumps, or command output.
- Ambiguous failure source for old failed workspaces -> Return a generic fallback failure detail for legacy rows instead of trying to infer false precision.
- Lifecycle rules becoming scattered -> Centralize failure creation/mapping helpers near provisioning progress or service code so tests cover each category.

## Migration Plan

1. Add the structured failure type and optional Workspace metadata field with serde defaults for old rows.
2. Update Workspace Catalog persistence and generated command bindings.
3. Update provisioning service failure paths to persist failure detail whenever they set lifecycle state to `failed`.
4. Keep retryable provider command errors as command errors where no durable failure truth is learned.
5. Update React provisioning rendering to consume structured failure detail after removing the old progress message field.
6. Add Rust tests for domain serialization, provisioning service lifecycle decisions, command error separation, and binding-safe response shape.

Rollback is limited because generated bindings may change. If needed before release, the optional field can remain in persisted metadata while React ignores it; Native should continue producing generic failed progress for failed workspaces.

## Resolved Questions

- The old `message` field is removed immediately from generated bindings.
- Sanitized worker diagnostics may be stored on Workspace failure detail only when they come from trusted worker error mapping; otherwise the diagnostic remains absent.
