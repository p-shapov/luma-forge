## Why

The Runtime Catalog currently carries runtime validation and diagnostic metadata that the app no longer needs to verify at runtime. Simplifying it will make runtime selection easier to reason about and keep build-only compatibility data on the worker/release side where it belongs.

## What Changes

- **BREAKING** Simplify `bundled/runtime-catalog.json` to contract families with `id` and nested `revisions[]` containing versioned immutable Provisioner Worker and Endpoint Worker image refs.
- **BREAKING** Remove implementation revisions, default implementation revision, display names, runtime metadata, image metadata, workspace overlay policy, runtime manifest compatibility, and release compatibility metadata from the app Runtime Catalog.
- **BREAKING** Change Workflow Presets to require a runtime contract id and version instead of selecting an implementation revision.
- **BREAKING** Persist only the selected runtime contract id, contract version, and immutable worker image refs in Workspace records.
- Remove Native runtime/image metadata validation and remove provider environment variables used only for runtime identity validation.
- Remove worker runtime checks that prove expected Python version, ComfyUI revision, overlay policy, dependency records, metadata files, image identity, or implementation revision.
- Keep worker build/release compatibility inputs in worker runtime recipes and Docker build arguments, not in the app Runtime Catalog.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `runtime-catalog`: Simplify the bundled catalog contract to contract id/version-to-image-ref selection only.
- `workspace-setup`: Resolve Workflow Preset runtime requirements by contract id/version and persist a simplified runtime snapshot.
- `workspace-provisioning`: Provision resources from simplified runtime snapshots without runtime identity environment injection.
- `provisioner-worker`: Stop requiring runtime catalog/image metadata for request validation and preparation.
- `endpoint-worker`: Stop requiring runtime identity environment values or image runtime contract validation.
- `prepared-runtime-environment`: Remove runtime validation/diagnostic fields from the prepared runtime manifest contract.
- `worker-deployment`: Update runtime recipe release catalog handling for the simplified catalog shape.

## Impact

- Affected catalog data: `bundled/runtime-catalog.json`, `bundled/workflow-catalog.json`.
- Affected native code: runtime domain models and validators, bundled catalog parser/tests, Workspace Setup, Workspace Catalog persistence compatibility, Workspace Provisioning, RunPod provider env/template matching, generated command bindings.
- Affected worker code: provisioner request schemas, config, runtime materialization/preparation, endpoint config/environment validation, Dockerfile metadata generation, runtime recipe release tooling and tests.
- Existing persisted Workspace records do not need backward compatibility because the app is not in production.
