## Why

Bundled catalog data is already exposed to the UI and persisted into Draft Workspaces, and upcoming provisioning will use that same data to drive remote Git, Docker, HTTP, provider, and filesystem operations. Current validation is uneven: model asset install paths are checked, but related workflow and profile fields are only partially validated or carry misleading metadata such as an unused Docker image digest.

This change hardens the bundled catalog contract with deterministic offline surface validation, so invalid local catalog shape is rejected before Workspace Setup accepts it, without turning catalog reads into external reachability or authenticity checks.

## What Changes

- Add surface validation for Workflow Catalog source fields, including ComfyUI Git runtime sources, Hugging Face model source shape, and Custom Node source/install fields.
- Require Custom Node checkout paths to be safe ComfyUI-relative paths under `custom_nodes/...`.
- Make `python_requirements_path` explicitly optional and define it as relative to the Custom Node checkout root when present.
- Add surface validation for Provisioning Profile and Endpoint Profile runtime/provider fields, including Docker image refs, mount paths, HTTP paths, ports, and enum-like provider settings.
- Remove `docker_image_digest` and the one-field Docker image wrapper from the v1 contract; keep `docker_image_ref` directly on worker runtime objects as the only Docker image identity field.
- Clarify that bundled catalog validation does not verify Docker image authenticity, Git reachability, Hugging Face asset existence, or Provider availability.
- Keep Provisioner Worker validation as the final remote filesystem safety boundary before provisioning writes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `workspace-setup`: Strengthen bundled Workflow Catalog, Provisioning Profile, and Endpoint Profile validation before catalog data is exposed or accepted for Workspace creation.
- `provisioner-worker`: Align worker request semantics with the catalog contract for optional Custom Node requirements paths and continued final path validation.

## Impact

- Affected native catalog contracts and validation:
  - `src-tauri/src/workspace/workspace_contracts.rs`
  - `src-tauri/src/domain/workflow.rs`
  - `src-tauri/src/domain/profiles.rs`
  - `src-tauri/src/domain/shared.rs`
  - `src-tauri/src/bundled/bundled_catalog_validator.rs`
  - `src-tauri/src/bundled/bundled_catalog_tests.rs`
- Affected bundled catalog data:
  - `resources/catalog/workflow-catalog.json`
  - `resources/catalog/provisioning-profiles.json`
  - `resources/catalog/endpoint-profiles.json`
- Affected generated/reference frontend contracts:
  - `src/generated/commands.ts`
  - `spec/reference/**`
- Affected worker schema/tests where Custom Node requirements optionality must remain aligned:
  - `workers/provisioner/src/provisioner_worker/schemas.py`
  - `workers/provisioner/tests/**`
