## 1. Runtime Catalog and Domain Model

- [x] 1.1 Add `bundled/runtime-catalog.json` with an initial exact generic ComfyUI Python/CUDA runtime contract entry and default implementation revision.
- [x] 1.2 Add domain models and validation for Runtime Catalog, runtime contract id/version, immutable implementation revision, immutable provisioner image ref, immutable endpoint image ref, and runtime metadata.
- [x] 1.3 Add bundled Runtime Catalog parser/reader tests for valid data, missing contracts, missing default implementation revisions, mutable image refs, malformed metadata, duplicate implementation revisions, and empty catalogs.
- [x] 1.4 Replace Workflow Preset `required_comfyui_source` with `required_runtime_contract` in domain models, bundled catalog JSON, parser tests, command bindings, and generated TypeScript references.
- [x] 1.5 Update Workspace Setup validation so every Workflow Preset runtime contract reference resolves through the bundled Runtime Catalog.
- [x] 1.6 Remove ComfyUI Git URL/revision validation from Workflow Preset validation while keeping immutable revision validation for workflow-declared Custom Nodes.
- [x] 1.7 Add resolved runtime contract implementation snapshot fields to Workspace domain, validation, SQLite persistence, migration handling, and Workspace Catalog tests.

## 2. Runtime Resolution in Provisioning

- [x] 2.1 Update Workspace creation to persist the resolved runtime contract implementation snapshot when creating a Draft Workspace.
- [x] 2.2 Update Workspace Provisioning config and provider calls so provisioning pod and endpoint template image refs come from the Workspace's resolved runtime implementation snapshot.
- [x] 2.3 Update Native build configuration so it validates only remaining non-image worker configuration values and no longer requires global worker image refs.
- [x] 2.4 Keep provisioning pod discovery/adoption keyed by stable Workspace pod name and network volume identity rather than provider-reported image identity.
- [x] 2.5 Update Provisioner Worker start request contracts to include resolved runtime contract and implementation metadata needed for worker-side validation.
- [x] 2.6 Update native provisioning progress mapping and tests to treat worker preparation as runtime materialization plus asset verification.
- [x] 2.7 Ensure native Workspace Provisioning never passes selected GPU information to base runtime or Custom Node dependency installation logic.
- [x] 2.8 Ensure placement validation does not pass selected GPU data into base runtime or Custom Node dependency resolution.

## 3. Runtime Archive Build

- [x] 3.1 Define the provisioner image runtime archive layout for `/workspace/.venv`, `/workspace/ComfyUI`, runtime contract metadata, and build-time dependency records.
- [x] 3.2 Add flat YAML runtime recipe input under a worker-owned recipe directory, with schema validation, declaring one contract id/version and its base runtime ingredients for image-pair builds.
- [x] 3.3 Update `workers/Dockerfile` provisioner target to install required system packages for the ComfyUI runtime archive.
- [x] 3.4 Add Docker build steps that create the runtime virtual environment with final `/workspace/.venv` prefix.
- [x] 3.5 Add Docker build steps that install fixed Python, PyTorch/CUDA-compatible packages, and ComfyUI base requirements into the runtime virtual environment.
- [x] 3.6 Add Docker build steps that fetch or copy the recipe-declared ComfyUI source, frontend/docs/templates, and base runtime metadata into the runtime archive.
- [x] 3.7 Add Docker build validation that records the runtime contract id/version, implementation revision metadata, Python version, ComfyUI revision, and base dependency records.
- [x] 3.8 Package the base runtime as a deterministic compressed tar archive and record archive metadata for release validation.

## 4. Provisioner Runtime Materialization

- [x] 4.1 Replace provisioner runtime preparation code that creates `/workspace/.venv` with staged extraction from the image-baked runtime archive.
- [x] 4.2 Replace provisioning-time ComfyUI Git checkout with materialization of the baked ComfyUI tree into `/workspace/ComfyUI`.
- [x] 4.3 Remove provisioning-time `pip install` for ComfyUI base requirements while preserving provisioning-time installation for preset-declared Custom Node requirements.
- [x] 4.4 Add or preserve Custom Node preparation that installs only the selected Workflow Preset's declared node sources and dependencies under the materialized ComfyUI root and venv.
- [x] 4.5 Validate incoming resolved runtime contract and implementation metadata against the Provisioner Worker image metadata before materialization.
- [x] 4.6 Keep Hugging Face model asset download behavior and ensure model install paths resolve under the materialized ComfyUI root.
- [x] 4.7 Update progress phases and UI-safe worker diagnostics to describe runtime materialization, Custom Node preparation, validation, and asset download without exposing secrets.
- [x] 4.8 Update runtime manifest creation to record resolved runtime contract metadata, selected implementation revision, image refs, plus preset-installed Custom Node revisions and dependency records.

## 5. Endpoint Runtime Validation

- [x] 5.1 Update endpoint prepared runtime manifest parsing to accept resolved runtime contract metadata, selected implementation revision, and the materialized image-baked runtime contract.
- [x] 5.2 Update endpoint environment validation to require `/workspace/.venv/bin/python`, `/workspace/ComfyUI/main.py`, required workflow files, required model files, and required Custom Node paths.
- [x] 5.3 Keep endpoint startup executing the manifest-declared workspace Python interpreter and workspace ComfyUI root.
- [x] 5.4 Add endpoint tests proving missing, mismatched, or invalid materialized runtime metadata fails without pip, Git, venv creation, or asset download repair attempts.

## 6. Runtime Recipe Release

- [x] 6.1 Replace separate publish workflows with a runtime recipe release workflow that builds provisioner and endpoint images from one runtime recipe.
- [x] 6.2 Add workflow validation that both images declare the same runtime contract id/version and release-assigned implementation revision metadata.
- [x] 6.3 Keep full provisioner runtime archive smoke validation out of CI because the provisioner image build installs the full ComfyUI dependency set.
- [x] 6.4 Add pair compatibility validation that proves both images declare matching runtime contract metadata.
- [x] 6.5 Add automation that proposes a new `bundled/runtime-catalog.json` entry or appends a new implementation revision from verified provisioner and endpoint image metadata after publication, advances the default implementation revision for future Workspaces, and opens a reviewed PR.
- [x] 6.6 Update deployment documentation for runtime recipe selection, immutable image refs, implementation revision increments, Runtime Catalog update PRs, and rollback.

## 7. Tests and Verification

- [x] 7.1 Update provisioner unit tests to expect base runtime archive extraction instead of ComfyUI Git checkout, venv creation, and base runtime pip install calls.
- [x] 7.2 Add provisioner tests for missing or mismatched resolved runtime contract or implementation metadata.
- [x] 7.3 Update endpoint tests to run against the new materialized runtime manifest shape.
- [x] 7.4 Update repository README references that still describe provisioning-time base runtime dependency installation into `/workspace/.venv`.
- [x] 7.5 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 7.6 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/runpod-endpoint`.
- [x] 7.7 Run `cargo test` from `src-tauri`.
- [x] 7.8 Run `cargo clippy --fix --allow-dirty --allow-staged` from `src-tauri`.
- [x] 7.9 Run `cargo fmt` from `src-tauri`.
- [x] 7.10 Run relevant Docker build validation for the runtime recipe image pair.
