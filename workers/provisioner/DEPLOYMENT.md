# Worker Deployment

Provisioner and RunPod Endpoint Worker images are deployed as a compatible runtime recipe pair through `.github/workflows/deploy-runtime-recipe.yml`.

## Triggers

- Push a release tag matching `runtime-recipe-v*`.
- Run the workflow manually and select one recipe, for example `workers/runtime-recipes/comfyui-python312-cu121.yaml`, plus an implementation revision.

## Registry

The workflow publishes to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/provisioner-worker`
- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Validation

The release workflow validates both worker packages, builds the provisioner image with the selected runtime recipe, builds the compatible endpoint image, and checks that both images declare matching runtime contract metadata. Full runtime archive extraction smoke checks are intentionally kept out of CI because building the provisioner runtime already installs the full ComfyUI dependency set.

## Runtime Catalog Update

After publishing a validated image pair, the workflow opens a reviewed PR that updates `bundled/runtime-catalog.json` with digest-pinned provisioner and endpoint image refs. Worker redeploys append a new immutable implementation revision under the existing contract and may advance `default_implementation_revision`.

Native must pass the selected immutable provisioner image ref into the temporary provisioning pod as `LUMA_FORGE_PROVISIONER_IMAGE_REF`. The value is not secret; it lets the Provisioner Worker verify that a start request matches the running image identity. The Provisioner Worker rejects startup configuration when this value is missing or blank.

## Rollback

Rollback by selecting a previously published immutable implementation revision from `bundled/runtime-catalog.json` as the default for future Workspaces, or by adding a reviewed replacement implementation revision. Existing Workspaces remain pinned to their persisted runtime implementation snapshot.
