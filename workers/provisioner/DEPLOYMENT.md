# Worker Deployment

The Provisioner Worker and RunPod Endpoint Worker are deployed by publishing separate container images from separate GitHub Actions workflows:

- `.github/workflows/deploy-provisioner-worker.yml`
- `.github/workflows/deploy-endpoint-worker.yml`

Both workflows build from the shared provider-neutral Dockerfile at `workers/Dockerfile`.
The provisioner deployment workflow also starts the built provisioner image and runs the container smoke test before authenticating to the registry or publishing tags.

## Triggers

- Push a release tag matching `provisioner-worker-v*` to run `Deploy Provisioner Worker`.
- Push a release tag matching `runpod-endpoint-worker-v*` to run `Deploy Endpoint Worker` for RunPod.
- Run either workflow manually from the GitHub Actions UI.

Manual dispatch for the endpoint workflow requires selecting an endpoint provider. RunPod is the only supported provider today. Manual dispatch publishes only the immutable SHA tag for the selected branch or commit and only for the selected worker image.

## Registry

The workflows publish to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/provisioner-worker`
- `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`

It uses GitHub Actions' built-in token and requires repository package write access. No custom registry secrets are required.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Validation

Provisioner deployment validation runs the Python test suite, builds the provisioner image, and runs the opt-in container smoke test against the built image tag:

```bash
cd workers/provisioner
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 \
  LUMA_FORGE_PROVISIONER_SMOKE_IMAGE=<built-image-tag> \
  PYTHONPATH=src python -m unittest tests.test_container_smoke
```

When `LUMA_FORGE_PROVISIONER_SMOKE_IMAGE` is omitted, the smoke test builds `luma-forge-provisioner:smoke` locally before running the container check.

## Published Tags

Every successful deployment publishes an immutable source tag for the selected image:

- `sha-<40-character-git-sha>`

Release tag deployments also publish the pushed Git tag, for example:

- `provisioner-worker-v1.0.0`
- `runpod-endpoint-worker-v1.0.0`

Use the `sha-<40-character-git-sha>` tag when a downstream system needs a reproducible image reference.

Provisioner and endpoint image refs should normally be selected from the same Git SHA so they use the same shared worker base assumptions. The runtime manifest validates the mounted volume environment shape, but it does not enforce image pairing.

## Rollback

Rollback by selecting previously published immutable `sha-<40-character-git-sha>` tags for the affected worker images and pointing downstream configuration at those image refs.
