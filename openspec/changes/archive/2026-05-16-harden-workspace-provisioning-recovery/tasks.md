## 1. Provider Discovery Contracts

- [x] 1.1 Extend `ProviderProvisioningGateway` with provider-neutral discovery methods for Workspace-correlated network volumes, endpoint templates, and serverless endpoints.
- [x] 1.2 Add provider-neutral discovery input contracts carrying Workspace id, expected provider placement, template, volume, image, port, mount path, and endpoint keep-alive context as needed.
- [x] 1.3 Keep existing provisioning pod discovery contract and identify the shared exact-one adoption behavior it should reuse after indeterminate pod creates.

## 2. RunPod Discovery Adapters

- [x] 2.1 Add RunPod client list/filter helpers for network volumes by deterministic Workspace-derived volume name and expected datacenter/size properties.
- [x] 2.2 Add RunPod client list/filter helpers for serverless templates by deterministic Workspace-derived template name and expected image, serverless, and mount path properties.
- [x] 2.3 Add RunPod client list/filter helpers for serverless endpoints by deterministic Workspace-derived endpoint name plus expected template id, volume id, GPU, and datacenter properties.
- [x] 2.4 Update RunPod response DTOs and mappers only inside `provider/runpod` so list responses expose the fields required for safe filtering without leaking raw provider payloads.
- [x] 2.5 Wire the new discovery helpers through `provider/registry.rs` using the existing `provider_resource_name` naming convention.
- [x] 2.6 Add provider tests for zero, one, and multiple discovery matches for volumes, templates, and endpoints.

## 3. Indeterminate Create Recovery

- [x] 3.1 Update network volume sync so pre-create discovery adopts exactly one safe matching volume and indeterminate create performs discovery before failing closed.
- [x] 3.2 Update provisioning pod sync so indeterminate pod create reruns pod discovery and adopts exactly one safe matching pod instead of returning a retryable command error.
- [x] 3.3 Update endpoint template sync so pre-create discovery adopts exactly one safe matching template and indeterminate create performs discovery before failing closed.
- [x] 3.4 Update serverless endpoint sync so pre-create discovery adopts exactly one safe matching endpoint and indeterminate create performs discovery before failing closed.
- [x] 3.5 Add service tests proving indeterminate create outcomes do not create a second volume, pod, template, or endpoint on the next sync.
- [x] 3.6 Add service tests proving exactly-one discovery persists the corresponding snapshot and zero-or-multiple discovery marks the Workspace `failed` with cleanup recovery detail.

## 4. Missing Resource Failure Handling

- [x] 4.1 Convert missing tracked network volume refresh into persisted `provider_resource_missing` failure detail for phase `creating_volume`.
- [x] 4.2 Convert missing tracked active provisioning pod refresh into persisted `provider_resource_missing` failure detail for phase `starting_provisioning_pod`.
- [x] 4.3 Convert missing tracked endpoint template refresh into persisted `provider_resource_missing` failure detail for phase `creating_endpoint_template`.
- [x] 4.4 Convert missing tracked serverless endpoint refresh into persisted `provider_resource_missing` failure detail for phase `creating_endpoint`.
- [x] 4.5 Add service tests for each missing-resource refresh path proving existing snapshots are retained and automatic recreation does not happen.

## 5. Cleanup And Cancellation Semantics

- [x] 5.1 Update shared known-resource cleanup to attempt per-workspace Provisioner Worker bearer token deletion regardless of active pod snapshot presence.
- [x] 5.2 Add cleanup tests proving token deletion happens when no active pod snapshot exists and missing tokens remain tolerated.
- [x] 5.3 Update `cancel` so coordinator conflict returns a retryable `ProviderOperationConflict` command error instead of success with unchanged Workspace metadata.
- [x] 5.4 Add service tests proving cancellation conflict does not delete provider resources, clear snapshots, delete tokens, or return success.
- [x] 5.5 Adjust frontend cancellation handling only if needed so retryable conflict is shown as an error and does not produce the cancellation success toast.

## 6. Verification

- [x] 6.1 Run targeted Workspace Provisioning and cleanup tests for the new recovery paths.
- [x] 6.2 Run `cargo test`.
- [x] 6.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 6.4 Run `cargo fmt`.
- [x] 6.5 If `src/` changes, run `bun run build` and `bun run lint --fix`.
