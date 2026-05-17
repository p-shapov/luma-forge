## 1. Catalog And Domain Models

- [x] 1.1 Replace `bundled/runtime-catalog.json` with `contracts[]` entries containing `id` and nested `revisions[]` with `version`, `provisioner_image_ref`, and `endpoint_image_ref`.
- [x] 1.2 Update `bundled/workflow-catalog.json` Workflow Presets to use `runtime_contract: { id, version }`.
- [x] 1.3 Simplify native runtime domain types to remove implementation revision, runtime metadata, image metadata, manifest compatibility, and overlay policy.
- [x] 1.4 Simplify Runtime Catalog resolution so Workspace Setup resolves a contract id/version pair to a minimal runtime image snapshot.
- [x] 1.5 Update native catalog parsing and catalog tests for the simplified shape.

## 2. Workspace Setup And Persistence

- [x] 2.1 Update Workflow Preset domain/contracts, validators, and generated command bindings for `runtime_contract: { id, version }`.
- [x] 2.2 Update Workspace creation to persist only runtime contract id, runtime contract version, provisioner image ref, and endpoint image ref.
- [x] 2.3 Remove legacy Workspace JSON compatibility for old runtime snapshot metadata.
- [x] 2.4 Update Workspace Catalog and command contract tests for the simplified runtime snapshot.

## 3. Workspace Provisioning And Provider Env

- [x] 3.1 Update Workspace Provisioning to read worker image refs from the simplified runtime image snapshot.
- [x] 3.2 Remove native injection of `LUMA_FORGE_PROVISIONER_IMAGE_REF` into provisioning pods.
- [x] 3.3 Remove endpoint template runtime identity env injection for `LUMA_FORGE_IMAGE_RUNTIME_ROOT`, `LUMA_FORGE_RUNTIME_CONTRACT_ID`, `LUMA_FORGE_RUNTIME_CONTRACT_VERSION`, `LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION`, and `LUMA_FORGE_ENDPOINT_IMAGE_REF`.
- [x] 3.4 Update endpoint template discovery/matching to rely on selected endpoint image ref, resource name, mount path, and provider-owned template settings.
- [x] 3.5 Remove provider pod image identity mapping from domain snapshots where it is only used for runtime validation.
- [x] 3.6 Update Workspace Provisioning and RunPod provider tests for the reduced provider environment.

## 4. Provisioner Worker

- [x] 4.1 Simplify Provisioner Worker request schemas to keep only runtime contract id/version and remove implementation revision, runtime metadata, image metadata, and provisioner image identity validation fields.
- [x] 4.2 Remove `LUMA_FORGE_PROVISIONER_IMAGE_REF`, runtime contract id/version, implementation revision, and image runtime root from Provisioner Worker startup configuration when they are only used for validation.
- [x] 4.3 Replace catalog-provided runtime layout and overlay policy usage with fixed worker constants.
- [x] 4.4 Remove image-baked runtime metadata file validation and base dependency record validation during materialization.
- [x] 4.5 Update prepared workspace manifest writing to include only workspace-specific runtime data and fixed runtime paths.
- [x] 4.6 Update Provisioner Worker unit tests, README, and deployment docs for the simplified runtime request/configuration.

## 5. Endpoint Worker

- [x] 5.1 Remove endpoint runtime identity configuration for contract id, contract version, implementation revision, endpoint image ref, and image runtime contract metadata.
- [x] 5.2 Replace endpoint runtime layout and ComfyUI path resolution with fixed image-baked runtime constants.
- [x] 5.3 Simplify prepared runtime manifest loading and validation to require only workspace-specific paths needed for generation.
- [x] 5.4 Remove image base dependency record validation from Endpoint Worker environment checks.
- [x] 5.5 Update Endpoint Worker tests and README for the reduced configuration surface.

## 6. Worker Deployment

- [x] 6.1 Simplify `workers/runtime-recipes/schema.json` and recipe metadata to remove implementation revision/default revision fields.
- [x] 6.2 Simplify `workers/runtime-recipes/release_tool.py` to upsert `contracts[].revisions[]` entries by contract id/version with provisioner and endpoint image refs.
- [x] 6.3 Remove runtime implementation revision generation and duplicate-revision checks from the release tool and workflow.
- [x] 6.4 Keep recipe build inputs for Python, ComfyUI, PyTorch, and base requirements on the worker side without copying them into `bundled/runtime-catalog.json`.
- [x] 6.5 Update `.github/workflows/deploy-runtime-recipe.yml`, Docker build args, worker Dockerfile metadata generation, and release tool tests for the simplified catalog update flow.

## 7. Frontend, Generated Types, And Verification

- [x] 7.1 Regenerate frontend command bindings after native contract changes.
- [x] 7.2 Update frontend usages of Workflow Preset runtime requirements and Workspace runtime snapshots.
- [x] 7.3 Run `cargo test`.
- [x] 7.4 Run `cargo clippy --fix --allow-dirty --allow-staged`.
- [x] 7.5 Run `cargo fmt`.
- [x] 7.6 Run `bun run build`.
- [x] 7.7 Run `bun run lint --fix`.
- [x] 7.8 Run relevant worker package tests for provisioner, endpoint, and runtime recipe tooling.
- [x] 7.9 Run `openspec status --change simplify-runtime-catalog-contract` and confirm the change is apply-ready.
