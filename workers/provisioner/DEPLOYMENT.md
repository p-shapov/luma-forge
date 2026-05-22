# Worker Deployment

Provisioner and RunPod Endpoint Worker images are no longer a ComfyUI runtime pair. The Provisioner Worker image is generic workspace-preparation infrastructure. Runtime recipe releases publish Endpoint Worker images that contain the ComfyUI runtime.

## Triggers

- Push a release tag matching `runtime-recipe-v*`.
- Run the workflow manually and select one recipe, for example `workers/runtime-recipes/comfyui-python312-cu121.yaml`.

Manual runtime releases append a new Runtime Catalog revision under the selected runtime contract id. The workflow resolves the next patch version from `bundled/runtime-catalog.json`, for example `1.0.0` to `1.0.1`, before worker validation, image builds, or publication. If the recipe declares a higher SemVer version than the next patch, the workflow uses the recipe version instead.

## Registry

The workflow publishes to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`

The generic provisioner image is selected from app/provider deployment configuration, not from the Runtime Catalog.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Validation

The release workflow validates worker packages and builds the endpoint image with the selected runtime recipe. The deterministic ComfyUI/PyTorch runtime is baked into the endpoint image under `/opt/luma-forge/runtime`; provisioning does not validate that runtime and prepares only workspace-specific directories and model assets on the RunPod network volume.

## Runtime Catalog Update

After publishing a validated endpoint image, the workflow opens a reviewed PR that appends the selected runtime contract id/version revision in `bundled/runtime-catalog.json` with a digest-pinned endpoint image ref. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that runtime contract id point at the new revision.

Native selects the immutable provisioner image ref from deployment configuration as the pod image and injects only the per-pod bearer token needed for worker authorization.

## Rollback

Rollback by updating `bundled/workflow-catalog.json` to point Workflow Presets back to a previously published Runtime Catalog revision, or by publishing a new reviewed revision. Existing Workspaces remain pinned to their persisted runtime image snapshot.
