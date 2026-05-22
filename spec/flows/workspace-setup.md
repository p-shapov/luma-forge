# Workspace Setup

## Goal

Create one local `Draft` Workspace Catalog entry from one selected Workflow Preset and configured Placement Plan.

## Scope

- Reads the Workflow Catalog and exposes selectable Workflow Presets to Client (React).
- Uses the selected Workflow Preset to derive the required base Persistent Storage Volume size.
- Uses the selected GPU Cloud Provider and live provider inventory to help the Client configure one Placement Plan.
- Validates the local provider key prerequisite, Workflow Preset existence, Placement Plan completeness, and Workflow Preset compatibility before persisting Workspace metadata.
- Persists one Workspace Catalog entry with a client-generated stable Workspace UUID and lifecycle state `Draft`.

## Non-goals

- Does not create, modify, attach, or delete Provider Resources.
- Does not perform provisioning job.

## Invariants

- Workflow Preset selection and Placement Plan configuration in Client (React) are temporary and non-authoritative until Workspace metadata is persisted by Native Layer (Rust / Tauri).
- Native Layer (Rust / Tauri) performs authoritative local provider key, catalog, and Placement Plan shape validation before creating Workspace metadata.
- Workspace Setup does not re-check selected data center or GPU membership against live provider inventory at creation time; later provider-owned flows reject unavailable or invalid placement values before mutating Provider Resources.
- Workspace creation does not revalidate provider identity or prove that the stored Provider API Key is still authorized by the Provider; live provider authorization is checked by Provider Inventory and later provider-owned flows.
- A successfully created Workspace must be marked as `Draft`.

## Actors

- User
- Client (React)
- Native Layer (Rust / Tauri)
- Provider

## Preconditions

- GPU Cloud Provider Setup has completed and can be used by this Workspace Setup attempt.
- Native Layer (Rust / Tauri) can read provider setup status, the secure keyring, the Workflow Catalog, and the Workspace Catalog.
- Native Layer can read the bundled Runtime Catalog and non-image worker runtime configuration needed to create a Workspace with a resolved runtime implementation snapshot.
- The Workflow Catalog contains at least one Workflow Preset supported by the current application build.
- The Provider can report data centers and GPU availability.

## Main Flow

1. User -> Client (React)
   Opens Workspace Setup.
   Result: Client prepares temporary Workspace Setup state.

2. Client (React) -> Native Layer (Rust / Tauri)
   Requests setup data:
   - provider setup status
   - selectable Workflow Presets
   Result: Native Layer receives a read-only setup request.

3. Native Layer (Rust / Tauri) -> local application state
   Reads provider setup status, secure keyring key presence, and Workflow Catalog.
   Result: Native Layer determines setup completeness and selectable Workflow Presets.

4. Native Layer (Rust / Tauri) -> Client (React)
   Returns redacted provider status and selectable Workflow Presets.
   Result: Client can render Workflow Preset choices.

5. User -> Client (React)
   Selects one Workflow Preset.
   Result: Client stores the selected Workflow Preset in temporary Workspace Setup state.

---

6. Client (React) -> Native Layer (Rust / Tauri)
   Requests placement options for the selected GPU Cloud Provider.
   Result: Native Layer receives a read-only placement request.

7. Native Layer (Rust / Tauri) -> local application state / Provider
   Validates that the provider is supported, setup is complete, and the required Provider API Key is present, then fetches current Provider inventory.
   Result: invalid setup is rejected before any Provider call; otherwise Native Layer receives current data-center and GPU availability.

8. Native Layer (Rust / Tauri) -> Client (React)
   Returns placement options:
   - available data centers
   - GPUs available per data center
   - provider maximum Persistent Storage Volume size when known
   Result: Client can render Placement Plan controls with minimum required base Persistent Storage Volume size, derived from the selected Workflow Preset.

9. User -> Client (React)
   Selects one data center, one GPU available in that data center, and optional additional Persistent Storage Volume size.
   Result: Client can assemble the Placement Plan with the final requested Persistent Storage Volume size.

10. Client (React) -> Client (React)
    Validates temporary setup state:
    - data center is selected
    - GPU is selected and belongs to the selected data center according to the latest placement options observed by the Client
    - Workflow Preset is selected
    - optional additional Persistent Storage Volume size is non-negative
    - final requested Persistent Storage Volume size satisfies the selected Workflow Preset minimum
    Result: Client blocks confirmation until the Placement Plan is complete.

---

11. User -> Client (React)
    Confirms Workspace creation.
    Result: Client generates one Workspace UUID for this creation attempt.

12. Client (React) -> Native Layer (Rust / Tauri)
    Requests Workspace metadata creation with:
    - client-generated Workspace UUID
    - selected GPU Cloud Provider identifier
    - selected Workflow Preset identifier
    - configured Placement Plan
    Result: Native Layer receives the complete Workspace Setup request.

13. Native Layer (Rust / Tauri) -> local application state
    Performs authoritative validation:
    - Workspace identifier is present and is a valid UUID
    - required Provider API Key is still present in secure keyring and can be parsed as a local secret value
    - configured Placement Plan is complete and catalog-compatible
    Result: Native Layer rejects stale or invalid requests before persisting Workspace metadata.

14. Native Layer (Rust / Tauri) -> Workspace Catalog
    Persists one complete Workspace metadata record and re-reads it from the Workspace Catalog.
    Result: Native Layer verifies that the complete record is durable and internally consistent.

15. Native Layer (Rust / Tauri) -> Client (React)
    Returns the created Workspace metadata.
    Result: Client receives the authoritative Workspace identifier and lifecycle state.

16. Client (React) -> User
    Shows the Workspace as created and `Draft`.
    Result: the Workspace is ready for Native-owned Workspace Provisioning.

## Success Result

- One complete Workspace Catalog entry exists and is owned by Native Layer (Rust / Tauri).
- The Workspace has a stable client-generated Workspace UUID.
- The Workspace references exactly one GPU Cloud Provider, selected Workflow Preset, and configured Placement Plan.
- The Workspace lifecycle state is `Draft`.
- Persistent Storage Volume, active Provisioning Pod, and Serverless Endpoint snapshots are present and empty.
- Existing unrelated Workspace Catalog entries are unchanged.
- No Provider Resources are created by this flow.
- Client (React) has discarded or superseded temporary Workspace Setup selection state with the authoritative Workspace metadata returned by Native Layer (Rust / Tauri).

## Failure Handling

- Workflow Catalog unavailable, unreadable, empty, or inconsistent
  - Native behavior: rejects Workflow Preset listing or Workspace creation before persistence.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: blocks Workspace Setup until catalog read succeeds.
- Workspace Catalog unavailable, unreadable, or inconsistent
  - Native behavior: rejects Workspace listing or Workspace creation before persistence.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: blocks Workspace Setup until catalog read succeeds.
- Unsupported or unconfigured GPU Cloud Provider
  - Native behavior: rejects before Provider lookup or Workspace persistence.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: routes user to GPU Cloud Provider Setup or recovery guidance.
- Missing or locally unreadable Provider API Key
  - Native behavior: rejects before Workspace persistence.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: routes user to GPU Cloud Provider Setup or recovery guidance.
- Provider API Key revoked or rejected by Provider
  - Native behavior: rejects Provider Inventory lookup or later provider-owned flows that need live provider authorization.
  - Mutation guarantee: Workspace Setup does not create, modify, attach, or delete Provider Resources.
  - Client behavior: routes user to GPU Cloud Provider Setup or recovery guidance.
- Provider API timeout/network error
  - Native behavior: rejects placement-option lookup as a transient provider failure.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: shows network error and allows retry.
- Invalid Placement Plan configuration
  - Native behavior: rejects Workspace creation before persistence when the plan is incomplete, references stale Workflow Preset data, or requests insufficient Persistent Storage Volume size.
  - Mutation guarantee: no Workspace Catalog mutation.
  - Client behavior: clears or marks temporary Placement Plan invalid and requires reselection.
- Duplicate request with the same Workspace UUID
  - Native behavior: rejects Workspace creation as a duplicate.
  - Mutation guarantee: at most one Workspace Catalog entry per Workspace UUID.
  - Client behavior: treats the duplicate as a failed create attempt and refreshes the Workspace Catalog when recovery is needed.

## Idempotency

Workflow Preset listing and placement-option lookup are read-only with respect to the Workspace Catalog and Provider Resources. They are safe to retry, but each retry may observe changed provider inventory.

Workspace metadata creation uses the client-generated Workspace UUID as a uniqueness key, not as a success-returning idempotency key. If Native Layer (Rust / Tauri) observes a duplicate request with the same Workspace UUID, it rejects the request with a duplicate Workspace error and leaves the existing Workspace Catalog entry unchanged.

## Cleanup / Rollback

Client (React) discards temporary Workspace Setup state when the user leaves setup or changes the selected GPU Cloud Provider.

No Provider Resource cleanup is required because this flow creates no Provider Resources. No Workspace cleanup is required before a complete Workspace Catalog entry is persisted.

After a complete Workspace Catalog entry is persisted and read back, it is authoritative even if Client (React) does not observe success. The user may continue with Workspace Provisioning or remove it through Workspace Resource Cleanup.

Partial or corrupt Workspace metadata must not be treated as `Ready` or used for provisioning. Recovery is cleanup-first in v1 when enough identifiers exist; otherwise Factory Reset may be required.

## See Also

### Flows

- [GPU Cloud Provider Setup](./gpu-cloud-provider-setup.md)

### Ubiquitous Language

- [GPU Cloud Provider](../ubiquitous-language/gpu-cloud-provider.md)
- [Provider API Key](../ubiquitous-language/provider-api-key.md)
- [Provider Resource](../ubiquitous-language/provider-resource.md)
- [Workflow Catalog](../ubiquitous-language/workflow-catalog.md)
- [Workflow](../ubiquitous-language/workflow.md)
- [Workflow Preset](../ubiquitous-language/workflow-preset.md)
- [Placement Plan](../ubiquitous-language/placement-plan.md)
- [Persistent Storage Volume](../ubiquitous-language/persistent-storage-volume.md)
- [Provisioning Pod](../ubiquitous-language/provisioning-pod.md)
- [Serverless Endpoint](../ubiquitous-language/serverless-endpoint.md)
- [Endpoint Worker](../ubiquitous-language/endpoint-worker.md)
- [Health Check](../ubiquitous-language/health-check.md)
- [Workspace](../ubiquitous-language/workspace.md)
- [Workspace Catalog](../ubiquitous-language/workspace-catalog.md)
- [Workspace Resource Cleanup](../ubiquitous-language/workspace-resource-cleanup.md)
- [Factory Reset](../ubiquitous-language/factory-reset.md)
