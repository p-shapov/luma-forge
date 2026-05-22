# Worker Deployment

Provisioner and RunPod Endpoint Worker images are deployed as a compatible runtime recipe pair through `.github/workflows/deploy-runtime-recipe.yml`.

## Triggers

- Push a release tag matching `runtime-recipe-v*`.
- Run the workflow manually and select one recipe, for example `workers/runtime-recipes/comfyui-python312-cu121.yaml`.

Manual releases append a new Runtime Catalog revision under the selected runtime contract id. The workflow resolves the next patch version from `bundled/runtime-catalog.json`, for example `1.0.0` to `1.0.1`, before worker validation, image builds, or publication. If the recipe declares a higher SemVer version than the next patch, the workflow uses the recipe version instead.

## Registry

The workflow publishes to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/provisioner-worker`
- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Validation

The release workflow validates both worker packages, builds the provisioner image with the selected runtime recipe, builds the compatible endpoint image, and checks that both images declare matching runtime contract metadata. The deterministic ComfyUI/PyTorch runtime is baked into both images under `/opt/luma-forge/runtime`; provisioning validates that image-baked runtime and prepares only workspace-specific directories on the RunPod network volume.

## Runtime Catalog Update

After publishing a validated image pair, the workflow opens a reviewed PR that appends the selected runtime contract id/version revision in `bundled/runtime-catalog.json` with digest-pinned provisioner and endpoint image refs. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that runtime contract id point at the new revision.

Native selects the immutable provisioner image ref as the pod image and injects only the per-pod bearer token needed for worker authorization.

## Rollback

Rollback by updating `bundled/workflow-catalog.json` to point Workflow Presets back to a previously published Runtime Catalog revision, or by publishing a new reviewed revision. Existing Workspaces remain pinned to their persisted runtime image snapshot.
