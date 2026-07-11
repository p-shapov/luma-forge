# Runtime Catalog Boundary Design

## Goal

Remove the standalone `application/catalog.rs` module. Its models describe workflow inputs to runtime provisioning, and its provider-specific models belong to RunPod.

## Runtime catalog ownership

`application/runtimes/catalog.rs` owns the provider-neutral catalog models:

- `CatalogRef`;
- `WorkflowSummary`;
- `WorkflowDefinition`;
- `RuntimeContractRequirements` as the runtime-provider dispatch enum.

Workspace continues to store `CatalogRef` for its selected workflow and to use `WorkflowDefinition` through `WorkflowCatalog`. This creates no new aggregate: workspace consumes the runtime catalog projection required to validate and provision its runtime.

`application/catalog.rs` is deleted and `application/mod.rs` no longer exports a top-level catalog module.

## RunPod catalog ownership

`application/runtimes/runpod/catalog.rs` owns:

- `RunpodContractRequirements`;
- `RunpodRuntimeDefinition`.

`RuntimeContractRequirements::Runpod` remains the provider-neutral dispatch variant. RunPod workflow code matches that variant directly. The provider-neutral `WorkflowDefinition` exposes no `runpod_*` method.

## Minimal RunPod definition

The single-use `RuntimePreset` and `RuntimeContract` wrappers are deleted. `RunpodRuntimeDefinition` contains only the values consumed by provisioning:

```rust
pub struct RunpodRuntimeDefinition {
    pub provisioner_image_ref: String,
    pub endpoint_image_ref: String,
}
```

The bundled RunPod catalog adapter still loads the referenced runtime preset and fails when it is missing or invalid. Its payload is not returned because current provisioning does not consume it. This preserves catalog validation behavior without retaining an unused application wrapper.

## Data flow

The bundled workflow adapter builds a provider-neutral `WorkflowDefinition` and provider dispatch values. During RunPod provisioning:

1. the service loads the workflow through `WorkflowCatalog`;
2. the RunPod service selects `RuntimeContractRequirements::Runpod` from the workflow requirements;
3. `RunpodRuntimeCatalog` resolves the preset and RunPod contract references;
4. the resolved provisioner and endpoint image references are passed to the provider calls.

No catalog loading, provider-call ordering, error mapping, or persistence behavior changes.

## Verification

Existing catalog, workspace, and RunPod tests are updated to the new module paths and flattened definition. No new integration tests are added.

Verification runs:

```sh
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

A final source audit confirms that `application::catalog`, `RuntimePreset`, `RuntimeContract`, and `runpod_requirements` are absent from active Rust code.
