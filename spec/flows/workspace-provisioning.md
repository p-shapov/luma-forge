# Workspace Provisioning

## Goal

Provision one saved `Draft` Workspace into a `Ready` Workspace by creating required Provider Resources, preparing the environment, and persisting the successful outcome in Workspace metadata.

## Scope

- Runs after Workspace Setup persists a complete `Draft` Workspace.
- Uses Native Layer (Rust / Tauri) as the workflow orchestrator and Client (React) only for initiate, sync, progress rendering, and cancellation.
- Creates required Provider Resources: Persistent Storage Volume, temporary provider-side Provisioning Pod, and Serverless Endpoint.
- Persists Provider Resource snapshots and reports Workspace Provisioning Progress.
- Deletes all created Workspace resources and returns the Workspace to `Draft` when the user cancels provisioning and cancellation cleanup succeeds.

## Non-goals

- Does not repair `Failed` Workspaces.

## Invariants

- Native Layer (Rust / Tauri) owns Workspace lifecycle mutations, Provider calls, and provisioning sequence decisions.
- Native Layer never exposes Provider API Keys to Client (React).
- Native Layer must run at most one provisioning sync operation per Workspace at a time.
- A Workspace must not become `Ready` unless required Provider Resources are persisted and still exist.
- Provider Resource snapshot status is the last persisted Native Layer observation and must be refreshed before readiness decisions.

## Actors

- User
- Client (React)
- Native Layer (Rust / Tauri)
- Provider
- Provisioner Worker

## Preconditions

- Workspace Setup completed and persisted a complete `Draft` Workspace.
- Workspace contains a valid Placement Plan with selected GPU for the Serverless Endpoint, data center, Persistent Storage Volume size, and selected Workflow Preset.
- GPU Cloud Provider Setup is complete.
- Provider API Key exists in the secure keyring.
- Native Layer can read and write the Workspace Catalog.
- Native Layer has non-image worker runtime configuration available through build-time configuration, and the Workspace contains a resolved runtime implementation snapshot with Provisioner Worker and Endpoint Worker image refs.

## Main Flow

1. Client (React) -> Native Layer (Rust / Tauri)
   Initiates provisioning for the Workspace identifier.
   Result: Native Layer receives the Workspace identifier and starts or resumes the provisioning workflow.

2. Native Layer (Rust / Tauri) -> Workspace Catalog
   Reads and validates the Workspace:
   - lifecycle state is `Draft`
   - Placement Plan is complete and valid
   - existing Provider Resource snapshots do not conflict with this Workspace
   Result: Workspace is valid for Provider mutation.

3. Native Layer (Rust / Tauri) -> Workspace Catalog
   Moves Workspace lifecycle to `Provisioning` when provider mutation work starts.
   Result: Workspace metadata records that provisioning is active or resumable.

4. Native Layer (Rust / Tauri) -> Client (React)
   Returns updated Workspace metadata and Workspace Provisioning Progress with status `running`.
   Result: Client enters provisioning progress state and starts the sync loop.

---

Sync loop begins after Workspace lifecycle becomes `Provisioning`.

5. Client (React) -> Native Layer (Rust / Tauri)
   Requests provisioning sync while Workspace Provisioning Progress status is `running`.
   Result: Native Layer receives one sync request.

6. Native Layer (Rust / Tauri) -> Workspace Catalog
   Reads Workspace metadata and derives the next safe provisioning activity.
   Result: one activity is selected for this sync iteration.

7. Native Layer (Rust / Tauri) -> Provider / Provisioner Worker / Workspace Catalog
   Performs at most one safe action for the selected activity:

   - Native Layer (Rust / Tauri) -> Provider / Workspace Catalog
     Sync persistent storage.
     Result: persistent storage state is reflected in Workspace metadata.

   - Native Layer (Rust / Tauri) -> Provider / Workspace Catalog
     Sync temporary provisioning compute.
     Result: temporary provider-side compute state is reflected in Workspace metadata.

   - Native Layer (Rust / Tauri) -> Provisioner Worker
     Observe environment preparation.
     Result: Workspace Provisioning Progress reflects current preparation progress.

   - Native Layer (Rust / Tauri) -> Provider / Workspace Catalog
     Finish temporary provisioning compute after environment preparation succeeds.
     Result: no active provider-side provisioning compute remains.

   - Native Layer (Rust / Tauri) -> Provider / Workspace Catalog
     Sync persistent runtime entry point.
     Result: runtime entry point state is reflected in Workspace metadata.

   - Native Layer (Rust / Tauri) -> Provider / Workspace Catalog
     Validate required Provider Resources.
     Result: Workspace becomes `Ready` when validation succeeds.

8. Native Layer (Rust / Tauri) -> Client (React)
   Returns authoritative Workspace metadata and Workspace Provisioning Progress.
   Result: Client repeats sync while status is `running`.

Sync loop ends when status is `completed`, `failed`, `cancelled`, or `idle`.

9. Client (React) -> User
   Shows the terminal Workspace state.
   Result: User sees `Ready`, cleanup path, cancellation result, or idle state.

## Sync Semantics

- Workspace metadata is authoritative. Workspace Provisioning Progress is derived from it and used only for rendering and sync-loop control.
- Sync may perform at most one safe workflow action, and only when that action can be derived from durable Workspace state.
- If another sync is already active for the Workspace, concurrent sync requests are read-only and return the latest persisted Workspace metadata and Progress.
- Provider-side changes are successful only after their matching Workspace metadata is durable.
- Sync may refresh Provider Resource snapshot status without creating new Provider Resources.
- Client (React) repeats sync while status is `running` and stops on `idle`, `completed`, `failed`, or `cancelled`.

## Success Result

- Workspace lifecycle is `Ready`.
- Required Provider Resources exist and their Workspace snapshots are ready.
- The prepared workspace volume contains the required model asset files and workspace directories needed for provisioning.
- No active Provisioning Pod remains; retained Provisioning Pod metadata is terminal.
- Workspace Provisioning Progress has status `completed` and no active phase.

## Failure Handling

- Invalid Workspace state
  - Native behavior: marks Workspace `Failed` and returns an error before Provider mutation.
  - Mutation guarantee: existing Provider Resource snapshots are preserved for Workspace Resource Cleanup.
  - Client behavior: renders returned Workspace metadata and requires cleanup before provisioning can start again.
- Provider API failure
  - Native behavior: persists known resource identifiers when available and marks Workspace `Failed` if provisioning cannot continue.
  - Mutation guarantee: known cleanup metadata is retained.
  - Client behavior: shows failed status and offers cleanup/retry path.
- Timeout after Provider request
  - Native behavior: checks Workspace metadata and Provider correlation before retrying creation.
  - Mutation guarantee: no duplicate Provider Resource is created blindly.
  - Client behavior: syncs or retries only through Native Layer.
- Provider Resource unavailable, failed, or indeterminate
  - Native behavior: updates the affected snapshot to `unknown` or `failed`, and marks Workspace `Failed` when provisioning cannot safely continue.
  - Mutation guarantee: known Provider Resource identifiers are retained; Workspace is not marked `Ready` from unknown status.
  - Client behavior: shows failed or non-ready state and cleanup path.
- Temporary provisioning compute failure
  - Native behavior: terminates active Provisioning Pod when possible, retains typed pod failure metadata, and marks Workspace `Failed`.
  - Mutation guarantee: known storage and pod metadata are retained for cleanup.
  - Client behavior: shows failed status and recovery action when safe to display.
- Runtime entry point or readiness validation failure
  - Native behavior: marks Workspace `Failed` when endpoint setup, prepared runtime volume validation, resource presence validation, or Provider validation fails.
  - Mutation guarantee: existing resource snapshots are retained.
  - Client behavior: shows non-ready state and cleanup path.
- Duplicate or concurrent sync request
  - Native behavior: returns existing or latest persisted Workspace metadata without duplicate provisioning work.
  - Mutation guarantee: at most one sync performs provisioning work for a Workspace at a time.
  - Client behavior: treats returned Workspace and Workspace Provisioning Progress as authoritative.
- Lost or stale local state
  - Native behavior: reads Workspace Catalog before mutation and rejects conflicting state.
  - Mutation guarantee: Workspace Catalog remains authoritative.
  - Client behavior: refreshes from returned Workspace metadata.
- App exit or crash during provisioning
  - Native behavior: resumes from durable Workspace checkpoints on the next sync when safe; otherwise searches for Workspace-correlated Provider Resources and marks Workspace `Failed`.
  - Mutation guarantee: known or discoverable Provider Resource identifiers are retained for cleanup.
  - Client behavior: offers cleanup and restart provisioning when automatic resume is unsafe.
- User cancel
  - Native behavior: deletes all Provider Resources created for the Workspace, clears snapshots, and returns lifecycle to `Draft` when cancellation cleanup succeeds.
  - Mutation guarantee: Workspace returns to a clean setup-ready state only if cleanup succeeds.
  - Client behavior: renders running cancellation progress until Native Layer returns `cancelled` status with `Draft` Workspace or `failed` status with `Failed` Workspace.

## Idempotency

The Workspace identifier is the stable idempotency key for the provisioning workflow.

Native Layer derives every sync action from durable Workspace checkpoints: lifecycle, Provider Resource snapshots, snapshot status, and recorded environment preparation state. Existing `creating`, `running`, or `ready` snapshots must be verified or observed, not recreated.

Provider resource creation must use a stable Workspace-derived correlation value when the Provider supports one. After timeout or lost response, Native Layer first inspects local snapshots and discoverable Provider resources. If it cannot identify exactly one safe match, it marks Workspace `Failed` and preserves cleanup metadata.

Client (React) may retry initiate, sync, or cancel, but must treat Native Layer responses as authoritative.

## Cleanup / Rollback

User cancellation is supported while Workspace Provisioning is in progress.

On successful cancellation, Native Layer deletes all Provider Resources created for the Workspace, clears provisioning snapshots, and returns the Workspace lifecycle to `Draft`.

If cleanup is incomplete or resource status cannot be confirmed, Native Layer marks Workspace `Failed` and retains known Provider Resource snapshots for Workspace Resource Cleanup.

Unexpected failures are cleanup-first: Native Layer preserves known identifiers and leaves recovery to Workspace Resource Cleanup unless the current operation can safely complete or undo its partial mutation.

## See Also

### Flows

- [Workspace Setup](./workspace-setup.md)

### Ubiquitous Language

- [Workspace](../ubiquitous-language/workspace.md)
- [Workspace Catalog](../ubiquitous-language/workspace-catalog.md)
- [Workspace Provisioning Progress](../ubiquitous-language/workspace-provisioning-progress.md)
- [GPU Cloud Provider](../ubiquitous-language/gpu-cloud-provider.md)
- [Provider API Key](../ubiquitous-language/provider-api-key.md)
- [Provider Resource](../ubiquitous-language/provider-resource.md)
- [Placement Plan](../ubiquitous-language/placement-plan.md)
- [Persistent Storage Volume](../ubiquitous-language/persistent-storage-volume.md)
- [Provisioning Pod](../ubiquitous-language/provisioning-pod.md)
- [Serverless Endpoint](../ubiquitous-language/serverless-endpoint.md)
- [Provisioner Worker](../ubiquitous-language/provisioner-worker.md)
- [Endpoint Worker](../ubiquitous-language/endpoint-worker.md)
- [Health Check](../ubiquitous-language/health-check.md)
- [Workspace Resource Cleanup](../ubiquitous-language/workspace-resource-cleanup.md)
