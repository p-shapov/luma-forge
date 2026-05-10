## 1. Workflow Setup

- [x] 1.1 Create `.github/workflows/deploy-provisioner-worker.yml` with release tag and `workflow_dispatch` triggers.
- [x] 1.2 Configure workflow permissions for repository checkout, package publishing, and minimal token scope.
- [x] 1.3 Add workflow inputs or variables for registry, image name, and optional manual channel tag.

## 2. Worker Validation

- [x] 2.1 Add a Python 3.12 validation job that installs the Provisioner Worker package dependencies.
- [x] 2.2 Run `PYTHONPATH=src python -m unittest discover -s tests` from `workers/provisioner`.
- [x] 2.3 Build the worker image from `workers/provisioner/Dockerfile` before any publish step.
- [x] 2.4 Ensure publish steps are skipped when validation or Docker build fails.

## 3. Image Publishing

- [x] 3.1 Add registry login using GitHub token context for GHCR and repository secrets for external registries.
- [x] 3.2 Generate image tags for commit SHA, release tag, and optional manual channel tag.
- [x] 3.3 Push the validated worker image to the configured registry with all generated tags.
- [x] 3.4 Ensure deployment logs do not print registry credentials, access tokens, provider API keys, or worker bearer tokens.

## 4. Documentation

- [x] 4.1 Document workflow triggers, required secrets or variables, and default GHCR behavior.
- [x] 4.2 Document produced image tags and how to identify the immutable commit SHA tag for a deploy.
- [x] 4.3 Document rollback by selecting a previously published immutable commit SHA tag.

## 5. Verification

- [x] 5.1 Run the worker unit test command locally from `workers/provisioner`.
- [x] 5.2 Run a local Docker build for `workers/provisioner/Dockerfile`.
- [x] 5.3 Run `openspec status --change add-worker-deploy-git-workflow` and confirm the change is apply-ready.
