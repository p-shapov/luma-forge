## Why

The bundled runtime provisioning change introduced a runtime catalog, runtime-specific worker images, and image-baked ComfyUI materialization, but the current implementation still has contract gaps that can make new workspaces fail to provision. This change stabilizes the runtime handoff between native provisioning, the provisioner image, the prepared runtime manifest, and the endpoint worker.

## What Changes

- Pass the selected immutable provisioner image ref into RunPod provisioning pods so worker-side runtime validation uses the configured runtime image identity.
- Accept RunPod pod responses that report container image identity using the provider's `image` field.
- Ensure the Provisioner Worker image exports the runtime contract id, version, implementation revision, and provisioner image ref expected by its runtime materializer.
- Materialize base dependency record files from the runtime archive into final workspace metadata paths and write endpoint-safe absolute manifest paths for those records.
- Extract or build runtime archives with compression that the Provisioner Worker can actually read in Python 3.12.

## Capabilities

- `prepared-runtime-environment`: Require prepared runtime manifests to advertise workspace-resolved dependency record paths that exist after materialization.
- `provisioner-worker`: Require provisioner runtime materialization to validate against image-exported runtime identity, read its configured archive format, and publish all base runtime records.
- `endpoint-worker`: Require endpoint validation to accept prepared manifests whose dependency record paths resolve under the mounted workspace.
- `workspace-provisioning`: Require native provisioning to pass selected runtime image identity into the provisioner pod and to parse provider pod image identity correctly.

## Impact

- Affected native code: RunPod pod request/response contracts, provider registry provisioning pod environment, workspace provisioning recovery behavior, and related tests.
- Affected worker code: `workers/Dockerfile`, Provisioner Worker config/materializer/manifest handling, Endpoint Worker prepared runtime validation, and worker tests.
- Affected catalogs and release automation: runtime archive metadata paths in `bundled/runtime-catalog.json` and `.github/workflows/deploy-runtime-recipe.yml`.
