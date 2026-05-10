## 1. Contract Updates

- [x] 1.1 Remove `docker_image_digest` and the one-field Docker image wrapper from reference contracts, native DTOs/domain types, generated frontend bindings, and bundled profile JSON.
- [x] 1.2 Make `python_requirements_path` optional in reference contracts, native Custom Node DTOs/domain types, generated frontend bindings, and worker schemas.
- [x] 1.3 Remove `src-tauri/src/domain/shared.rs` and redundant Docker image wrapper types after the digest field is removed.

## 2. Native Catalog Validation

- [x] 2.1 Add bundled catalog validator helpers for safe relative paths, Custom Node checkout paths under `custom_nodes/...`, safe repo-relative file paths, URL-shaped strings, Hugging Face repo ids, absolute normalized POSIX mount paths excluding `/`, HTTP paths, Docker image refs, and supported enum-like values.
- [x] 2.2 Extend Workflow Catalog validation for ComfyUI Git source fields, Hugging Face model source fields, existing model asset install paths, and Custom Node source/install fields.
- [x] 2.3 Extend Provisioning Profile validation for Docker image refs, worker mount paths, status endpoint protocol/path/port, RunPod mount paths, exposed ports, cloud type, and environment shape.
- [x] 2.4 Extend Endpoint Profile validation for Docker image refs, worker HTTP paths/ports, RunPod mount paths, scaling values, scaler type, and environment shape.
- [x] 2.5 Ensure all bundled catalog validation remains offline and does not call Docker registries, Git, Hugging Face, RunPod, or worker endpoints.

## 3. Worker Alignment

- [x] 3.1 Align Provisioner Worker request parsing so absent Custom Node requirements paths are accepted and blank present requirements paths are rejected.
- [x] 3.2 Ensure Provisioner Worker checkout path validation requires Custom Node paths to resolve under the prepared ComfyUI `custom_nodes` directory.
- [x] 3.3 Ensure Provisioner Worker requirements path validation resolves paths relative to the Custom Node checkout root before dependency installation.

## 4. Tests

- [x] 4.1 Add native bundled Workflow Catalog validation tests for malformed Git source fields, malformed Hugging Face source fields, unsafe model source file paths, unsafe Custom Node checkout paths, checkout paths outside `custom_nodes/...`, optional requirements paths, and unsafe requirements paths.
- [x] 4.2 Add native bundled Provisioning Profile validation tests for malformed Docker refs, unsafe mount paths, root-only mount paths, malformed HTTP status paths, invalid ports, unsupported cloud type values, and invalid exposed port relationships.
- [x] 4.3 Add native bundled Endpoint Profile validation tests for malformed Docker refs, unsafe mount paths, malformed health/invoke paths, invalid worker ports, unsupported scaler types, and inconsistent scaling values.
- [x] 4.4 Add worker schema/preparer tests for absent requirements paths, blank requirements paths, safe checkout paths under `custom_nodes/...`, checkout paths outside `custom_nodes/...`, and requirements paths escaping the checkout root.

## 5. Verification

- [x] 5.1 Regenerate generated TypeScript command bindings after native contract changes.
- [x] 5.2 Run `cargo test`.
- [x] 5.3 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 5.4 Run `cargo fmt`.
- [x] 5.5 Run Provisioner Worker tests.
- [x] 5.6 Run `bun run build`.
- [x] 5.7 Run `bun run lint --fix`.
- [x] 5.8 Run OpenSpec validation/status checks for `validate-bundled-catalog-surface-contracts`.
