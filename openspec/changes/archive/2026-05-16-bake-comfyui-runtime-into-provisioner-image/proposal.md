## Why

Provisioning currently installs the deterministic ComfyUI base Python runtime on each workspace volume, which makes every new workspace repeat expensive dependency setup and leaves base runtime reproducibility dependent on provisioning-time package resolution. LumaForge also needs a stable way to keep older Workflow Presets and Workspaces working as newer ComfyUI runtimes are introduced; Workflow Presets should require named runtime contracts, while verified worker images implement those contracts.

## What Changes

- **BREAKING**: Replace the prepared runtime format based on provisioning-time `/workspace/.venv` creation and base runtime dependency installation with a Docker-build-produced ComfyUI runtime archive.
- **BREAKING**: Replace Workflow Preset ComfyUI Git revision selection with `required_runtime_contract` references.
- Add a bundled runtime catalog that maps runtime contract ids and versions to verified runtime metadata plus immutable provisioner/endpoint image-pair implementation revisions.
- Store the resolved runtime contract and selected implementation snapshot on each Workspace so existing Workspaces remain pinned even when bundled catalogs add newer runtime versions or implementation revisions.
- Build the Provisioner Worker image from a runtime recipe that declares the runtime contract id/version, fixed Python, PyTorch/CUDA-compatible packages, ComfyUI checkout, ComfyUI frontend/docs/templates, and base ComfyUI requirements.
- Build the runtime virtual environment with the final `/workspace/.venv` prefix and package the base runtime as a deterministic archive in the Provisioner Worker image so it can be extracted onto the mounted workspace volume without path relocation.
- Change provisioning so it does not run `pip install` for the base ComfyUI runtime, create a fresh virtual environment, or clone ComfyUI from Git during workspace preparation.
- Change provisioning to extract the baked runtime archive into the mounted workspace volume, install or verify Workflow Preset Custom Nodes, write runtime metadata, and download or verify workspace assets.
- Keep the Endpoint Worker lightweight by continuing to run ComfyUI from the mounted workspace runtime after provisioning materializes it.
- Replace separate worker publishing with a runtime recipe release workflow that builds and validates compatible provisioner/endpoint image pairs, publishes immutable image refs, and proposes a new Runtime Catalog entry or implementation revision.
- Ensure selected GPU choice never determines base runtime or Custom Node Python dependency installation; v1 GPU handling remains provider placement validation and does not select Python packages.

## Capabilities

### New Capabilities

- `runtime-catalog`: Defines bundled runtime contracts, runtime recipes, immutable worker image implementation revisions, default implementation resolution, and workspace-pinned resolved runtime implementation snapshots.

### Modified Capabilities

- `native-build-configuration`: Stop treating global build-time worker image refs as authoritative deployment inputs; image refs now come from runtime catalog implementation snapshots.
- `prepared-runtime-environment`: Replace volume-local base dependency installation requirements with a materialized image-baked runtime archive and metadata contract.
- `provisioner-worker`: Change preparation from Git/venv/pip base runtime installation to runtime archive extraction, Workflow Preset Custom Node preparation, verification, and workspace asset download.
- `endpoint-worker`: Continue forbidding repair/install behavior while validating and running the materialized image-baked runtime from the mounted workspace.
- `worker-deployment`: Replace independent worker image publishing with runtime-recipe-oriented builds that publish compatible image pairs and update the Runtime Catalog through review.
- `workspace-provisioning`: Resolve provisioning image refs from the Workspace's resolved runtime contract implementation snapshot, recover provisioning pods by provider ownership identity, and clarify that preparation means materializing and verifying the baked base runtime, Custom Nodes, and assets.
- `workspace-setup`: Resolve Workflow Preset runtime contract references through the bundled runtime catalog, persist the resolved runtime implementation snapshot, and clarify that GPU placement validation does not select runtime dependencies.

## Impact

- Affected bundled catalogs: `bundled/workflow-catalog.json`, new `bundled/runtime-catalog.json`, catalog parsers, domain models, generated command bindings, and Workspace persistence.
- Affected Docker/deployment: `workers/Dockerfile`, runtime recipe inputs, unified runtime recipe release workflow, image-pair metadata validation, deployment documentation.
- Affected Provisioner Worker code: runtime archive extraction/materialization, runtime metadata, validation, progress phases, tests, and documentation under `workers/provisioner/`.
- Affected Endpoint Worker code: prepared runtime manifest parsing and validation, ComfyUI startup assumptions, tests, and documentation under `workers/runpod-endpoint/`.
- Affected Native code/specs: runtime catalog reading, native build configuration, workspace setup validation, workspace snapshot persistence, workspace provisioning image resolution, placement validation contracts, and generated command behavior.
- Existing pre-production prepared workspace volumes using the old provisioning-time `/workspace/.venv` format must be reprovisioned.
