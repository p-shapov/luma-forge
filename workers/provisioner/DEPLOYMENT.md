# Worker Deployment

The Provisioner Worker and RunPod Endpoint Worker are deployed by publishing separate container images from separate GitHub Actions workflows:

- `.github/workflows/deploy-provisioner-worker.yml`
- `.github/workflows/deploy-endpoint-worker.yml`

Both workflows build from the shared provider-neutral Dockerfile at `workers/Dockerfile`.

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
