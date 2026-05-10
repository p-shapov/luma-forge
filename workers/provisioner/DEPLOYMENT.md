# Provisioner Worker Deployment

The Provisioner Worker is deployed by publishing a container image from GitHub Actions. The workflow lives at `.github/workflows/deploy-provisioner-worker.yml` and builds `workers/provisioner/Dockerfile`.

## Triggers

- Push a release tag matching `worker-v*` or `provisioner-worker-v*`.
- Run `Deploy Provisioner Worker` manually from the GitHub Actions UI.

Manual dispatch has no required inputs. It publishes only the immutable SHA tag for the selected branch or commit.

## Registry

The workflow publishes to GitHub Container Registry:

- `ghcr.io/<owner>/<repo>/provisioner-worker`

It uses GitHub Actions' built-in token and requires repository package write access. No custom registry secrets are required.

Do not store registry credentials, provider API keys, or worker bearer tokens in repository files. The deployment workflow reads authentication only from GitHub Actions token context.

## Published Tags

Every successful deployment publishes an immutable source tag:

- `sha-<40-character-git-sha>`

Release tag deployments also publish the pushed Git tag, for example:

- `worker-v1.0.0`
- `provisioner-worker-v1.0.0`

Use the `sha-<40-character-git-sha>` tag when a downstream system needs a reproducible image reference.

## Rollback

Rollback by selecting a previously published immutable `sha-<40-character-git-sha>` image tag and pointing downstream provisioning configuration at that tag.
