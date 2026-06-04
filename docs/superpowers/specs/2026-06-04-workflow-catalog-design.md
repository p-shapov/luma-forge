# Workflow Catalog Design

## Summary

Add a native backend workflow catalog service that can list bundled workflow presets and look up one workflow by id. The current implementation source is the bundled JSON catalog, but the service boundary must allow a future backend-backed reader without changing catalog consumers.

This design does not add Tauri commands, generated TypeScript bindings, frontend UI, backend/API readers, caching, configurable catalog sources, or resolved image response DTOs.

## Goals

- Provide a scalable service foundation for workflow catalog reads.
- Keep the current implementation backed by bundled catalog files only.
- Share validation across bundled readers and any future reader implementation.
- Keep validation in the workflow catalog service layer, not in domain modules.
- Keep endpoint and provisioner contract catalog support inside the workflow catalog module instead of creating separate services.

## Module Layout

Create one module under `src-tauri/src/workflow_catalog/`:

- `mod.rs`
  - Exports the public service API.
  - Exports the reader traits needed to construct the service.
  - Re-exports shared error/result types used by callers.

- `service.rs`
  - Defines `WorkflowCatalogService`.
  - Provides:
    - `get_workflows`
    - `get_workflow_by_id`
  - Loads workflow presets through `WorkflowCatalogReader`.
  - Loads endpoint and provisioner contract catalogs through their reader traits.
  - Calls shared validators before returning workflow data.

- `reader.rs`
  - Defines:
    - `WorkflowCatalogReader`
    - `EndpointContractCatalogReader`
    - `ProvisionerContractCatalogReader`
    - `BundledWorkflowCatalogReader`
    - `BundledEndpointContractCatalogReader`
    - `BundledProvisionerContractCatalogReader`
  - Bundled readers use `include_str!` for:
    - `bundled/workflow-catalog.json`
    - `bundled/endpoint-contracts.json`
    - `bundled/provisioner-contracts.json`
  - Bundled readers deserialize JSON into existing domain structs.
  - The workflow reader may use a private DTO for the top-level `workflow_presets` JSON wrapper, then return `Vec<WorkflowPreset>`.

- `validation.rs`
  - Validates endpoint contract catalogs.
  - Validates provisioner contract catalogs.
  - Validates workflow presets against both contract catalogs.
  - Contains shared validation logic used by the service, independent of reader source.

Register the module from `src-tauri/src/lib.rs` with `pub mod workflow_catalog;`.

## Interfaces

Reader traits should be narrow and read-only:

```rust
pub trait WorkflowCatalogReader {
    fn read_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>>;
}

pub trait EndpointContractCatalogReader {
    fn read_endpoint_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}

pub trait ProvisionerContractCatalogReader {
    fn read_provisioner_contract_catalog(&self) -> WorkflowCatalogResult<RuntimeCatalog>;
}
```

The service should own orchestration:

```rust
pub struct WorkflowCatalogService<W, E, P> {
    workflow_reader: W,
    endpoint_contract_reader: E,
    provisioner_contract_reader: P,
}

impl<W, E, P> WorkflowCatalogService<W, E, P>
where
    W: WorkflowCatalogReader,
    E: EndpointContractCatalogReader,
    P: ProvisionerContractCatalogReader,
{
    pub fn get_workflows(&self) -> WorkflowCatalogResult<Vec<WorkflowPreset>>;

    pub fn get_workflow_by_id(
        &self,
        workflow_id: &str,
    ) -> WorkflowCatalogResult<Option<WorkflowPreset>>;
}
```

`get_workflow_by_id` returns `Ok(None)` when the id is absent. Missing ids are normal lookup results, not errors.

## Validation

Validation belongs in `workflow_catalog::validation`, not in `domain`.

Endpoint and provisioner contract catalog validators should check the runtime catalog shape used by `bundled/endpoint-contracts.json` and `bundled/provisioner-contracts.json`:

- catalog has at least one contract
- contract ids are non-blank and unique
- each contract has at least one revision
- revision versions are non-blank
- revision image refs are non-blank

Workflow validation should check:

- workflow list is not empty
- workflow ids are non-blank and unique
- workflow versions and names are non-blank
- `required_base_volume_size_bytes` is greater than zero
- each workflow has at least one provider requirement
- provider requirement contract references resolve in the endpoint and provisioner catalogs
- model asset ids and names are non-blank
- Hugging Face repository ids have exactly `owner/repository` format with safe characters
- model asset source file paths and install paths are safe relative paths
- model asset revisions are non-blank

The service validates all three catalogs before returning workflows. Future backend-backed readers must pass through the same validators after mapping backend data into domain structs.

## Error Handling

Use a workflow catalog specific error type, for example `WorkflowCatalogError`, with cases for:

- parse failure
- validation failure

The error does not need to expose detailed invalid catalog internals to callers at this stage. Tests can assert the error category.

## Testing

Add focused Rust tests for:

- bundled workflow reader deserializes `bundled/workflow-catalog.json`
- bundled endpoint contract reader deserializes `bundled/endpoint-contracts.json`
- bundled provisioner contract reader deserializes `bundled/provisioner-contracts.json`
- service returns all bundled workflows
- service returns `Some(workflow)` for a known workflow id
- service returns `None` for an unknown workflow id
- validation rejects duplicate workflow ids
- validation rejects missing endpoint contract references
- validation rejects missing provisioner contract references
- validation rejects invalid model asset paths

Run native verification:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Do not run command codegen for this slice because Tauri command contracts are out of scope.

## Approved Scope Boundaries

In scope:

- `workflow_catalog` service module.
- Reader traits for workflow, endpoint contract, and provisioner contract catalogs.
- Bundled reader implementations for all three bundled files.
- Shared validation in the service module.
- Tests for bundled readers, service behavior, and validation.

Out of scope:

- Tauri commands.
- Generated frontend bindings.
- Frontend UI.
- Backend/API reader implementation.
- Caching, pagination, auth, remote refresh, or configurable source selection.
- Resolved endpoint/provisioner image refs in workflow responses.
- Domain-level validators.
