# Worker Deployment

Provisioner and RunPod Endpoint Worker images are released separately. The Provisioner Worker image is generic workspace-preparation infrastructure. Runtime contract releases publish Endpoint Worker images for the selected endpoint runtime contract.

## Triggers

- Push a provisioner release tag matching `provisioner-worker-v*`, or run `Deploy Provisioner Worker` manually, to publish the generic provisioner image.
- Push a runtime contract release tag matching `runtime-contract-v*`, or run `Deploy Runtime Contract` manually and select one contract, for example `workers/runtime-contracts/comfyui-python312-cu121.yaml`.

Manual runtime contract releases append a new Runtime Catalog revision under the selected runtime contract id. The workflow resolves the next patch version from `bundled/runtime-catalog.json`, for example `1.0.0` to `1.0.1`, before endpoint worker validation, image builds, or publication. If the contract declares a higher SemVer version than the next patch, the workflow uses the contract version instead.

## Registry

The workflow publishes to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`
- `ghcr.io/<owner>/<repo>/provisioner-worker`

The generic provisioner image is selected from app/provider deployment configuration, not from the Runtime Catalog.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Validation

The provisioner workflow validates only the provisioner package and builds the generic provisioner image without runtime contract build arguments. The runtime contract workflow validates the endpoint package and runtime contract tooling, builds the endpoint image with the selected contract dependencies, and does not require live ComfyUI execution.

## Runtime Catalog Update

After publishing a validated endpoint image, the workflow opens a reviewed PR that appends the selected runtime contract id/version revision in `bundled/runtime-catalog.json` with a digest-pinned endpoint image ref. The same PR updates `bundled/workflow-catalog.json` so Workflow Presets using that runtime contract id point at the new revision.

Native selects the immutable provisioner image ref from deployment configuration as the pod image and injects only the per-pod bearer token needed for worker authorization.

## Rollback

Rollback by updating `bundled/workflow-catalog.json` to point Workflow Presets back to a previously published Runtime Catalog revision, or by publishing a new reviewed revision. Existing Workspaces remain pinned to their persisted runtime image snapshot.
