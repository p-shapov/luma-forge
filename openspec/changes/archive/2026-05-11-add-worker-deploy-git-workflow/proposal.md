## Why

The Provisioner Worker already exists as a containerized component, but the repository has no authoritative Git-based deployment path for publishing worker images. A GitHub Actions workflow is needed now so worker releases can be built, verified, tagged, and deployed reproducibly instead of relying on local manual Docker commands.

## What Changes

- Add a GitHub Actions workflow for deploying the Provisioner Worker from Git.
- Build the worker container from `workers/provisioner/Dockerfile`.
- Run worker validation before publishing an image.
- Publish immutable worker images using commit SHA tags and branch or release tags appropriate for the triggering ref.
- Keep registry credentials in GitHub Actions secrets and out of repository files, logs, and generated artifacts.
- Document the required repository secrets and the manual dispatch inputs needed for controlled deployment.

## Capabilities

### New Capabilities
- `worker-deployment`: Defines the GitHub Actions deployment contract for validating, building, tagging, and publishing Provisioner Worker container images from Git.

### Modified Capabilities

## Impact

- Affected systems: GitHub Actions, container image registry, Provisioner Worker Docker build.
- Affected files are expected under `.github/workflows/`, `workers/provisioner/`, and worker deployment documentation.
- No Tauri command API, frontend route, provider setup flow, or worker runtime HTTP API behavior is expected to change.
