## 1. Worker Project Structure

- [x] 1.1 Create `/workers/provisioner` with Python package metadata, application entrypoint, and test layout.
- [x] 1.2 Add worker dependency definitions for the HTTP server, request validation, tests, and public file downloads.
- [x] 1.3 Add a container image definition that runs the worker and exposes the configured status port.
- [x] 1.4 Document local worker run, test, and container build commands in the worker directory.

## 2. Worker API Contract

- [x] 2.1 Define request and response schemas for `POST /start`, `POST /cancel`, and `GET /status`.
- [x] 2.2 Implement idle startup behavior with no preparation work before `/start`.
- [x] 2.3 Implement `/start` validation for job id, selected Workflow Preset payload, mounted workspace path, supported sources, and safe install paths.
- [x] 2.4 Implement conflict handling so `/start` rejects a second active job without queueing or replacing work.
- [x] 2.5 Implement `/status` responses with active job id, status, phase, optional progress percentage, UI-safe diagnostics, version, and updated timestamp.
- [x] 2.6 Implement `/cancel` behavior for active, terminal, idle, and unmatched job cases.

## 3. Environment Preparation

- [x] 3.1 Implement safe path normalization and validation for all ComfyUI-relative install paths.
- [x] 3.2 Implement ComfyUI Git checkout into the mounted workspace volume from the selected Workflow Preset runtime source.
- [x] 3.3 Implement ComfyUI dependency installation and failure reporting.
- [x] 3.4 Implement Custom Node Git checkout and dependency installation for presets with required Custom Nodes.
- [x] 3.5 Implement public Hugging Face model asset downloads using repository id, file path, revision, and explicit install path.
- [x] 3.6 Implement filesystem validation before terminal success for required ComfyUI files, Custom Nodes, dependencies, and model assets.
- [x] 3.7 Ensure all subprocess, download, and validation failures produce terminal worker failure with UI-safe diagnostics.

## 4. Catalog and Contracts

- [x] 4.1 Extend Model Asset contract types with explicit ComfyUI-relative install path data.
- [x] 4.2 Update bundled Workflow Catalog data so every model asset declares an explicit install path.
- [x] 4.3 Update bundled catalog validation to reject missing, blank, absolute, parent-traversing, or otherwise unsafe model asset install paths.
- [x] 4.4 Update generated/reference TypeScript contract sketches where they describe Model Asset or Workflow Preset shape.
- [x] 4.5 Keep existing Workspace Setup behavior unchanged except for validating and returning explicit model asset install paths.

## 5. Tests

- [x] 5.1 Add worker API tests for idle status, accepted start, invalid start, concurrent start conflict, cancellation, success status, and failure status.
- [x] 5.2 Add worker path-safety tests for relative paths, blank paths, absolute paths, parent traversal, and paths resolving outside the ComfyUI root.
- [x] 5.3 Add worker preparation tests with mocked Git, dependency installation, and Hugging Face downloads.
- [x] 5.4 Add catalog validation tests for model asset install path requirements.
- [x] 5.5 Add container smoke verification that the image starts the worker server and reports idle status.

## 6. Verification

- [x] 6.1 Run the worker unit test suite.
- [x] 6.2 Run worker formatting and linting commands selected for the Python project.
- [x] 6.3 Build the provisioner worker container image.
- [x] 6.4 Run `cargo test` for native catalog contract changes.
- [x] 6.5 Run `cargo clippy --fix --allow-dirty --allow-staged` for native catalog changes.
- [x] 6.6 Run `cargo fmt`.
- [x] 6.7 Run `bun run build` after generated/reference frontend contract changes.
- [x] 6.8 Run `bun run lint --fix` after generated/reference frontend contract changes.
