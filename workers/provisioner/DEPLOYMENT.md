# Provisioner Worker Deployment

The Provisioner Worker is deployed by publishing a container image from GitHub Actions. The workflow lives at `.github/workflows/deploy-provisioner-worker.yml` and builds `workers/provisioner/Dockerfile`.

## Triggers

- Push a release tag matching `worker-v*` or `provisioner-worker-v*`.
- Run `Deploy Provisioner Worker` manually from the GitHub Actions UI with `workflow_dispatch`.

Manual dispatch accepts:

- `registry`: container registry host. Defaults to `ghcr.io`.
- `image_name`: image path without registry. Defaults to `<owner>/<repo>/provisioner-worker`.
- `channel_tag`: optional mutable tag such as `staging`.

## Registry Configuration

The default GHCR path uses GitHub Actions' built-in token and requires no extra registry secrets beyond repository package write access.

For an external registry, configure these repository secrets:

- `WORKER_REGISTRY_USERNAME`
- `WORKER_REGISTRY_PASSWORD`

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads credentials only from GitHub Actions token context or repository secrets.

## Published Tags

Every successful deployment publishes an immutable source tag:

- `sha-<40-character-git-sha>`

Release tag deployments also publish the pushed Git tag, for example:

- `worker-v1.0.0`
- `provisioner-worker-v1.0.0`

Manual deployments publish `channel_tag` when one is supplied, for example:

- `staging`

Use the `sha-<40-character-git-sha>` tag when a downstream system needs a reproducible image reference.

## Rollback

Rollback by selecting a previously published immutable `sha-<40-character-git-sha>` image tag and pointing downstream provisioning configuration at that tag.

If a mutable channel tag needs to move back to a known-good image, re-run the manual workflow for the known-good Git ref with the same `channel_tag`. The workflow will republish the channel tag after validation succeeds.
