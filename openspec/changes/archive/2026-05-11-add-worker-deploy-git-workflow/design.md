## Context

The Provisioner Worker is a Python service packaged by `workers/provisioner/Dockerfile`. The repository documents local Docker build and smoke-test commands, but it does not yet provide a GitHub Actions workflow that turns a reviewed Git ref into a published worker image. LumaForge v1 has no hosted backend; deployment for this component means publishing a verified container image that remote GPU provisioning can reference later.

The workflow must fit the current repository layout, keep secrets in GitHub Actions, and avoid changing the worker HTTP API or the native provisioning lifecycle as part of this change.

## Goals / Non-Goals

**Goals:**

- Add a GitHub Actions workflow that validates, builds, tags, and publishes the Provisioner Worker image.
- Support automatic deployment from release tags and controlled manual deployment through `workflow_dispatch`.
- Use immutable commit SHA image tags for every published image.
- Keep registry authentication outside committed files and avoid printing credentials in logs.
- Document the secrets, variables, triggers, and rollback process required to operate the workflow.

**Non-Goals:**

- Add a hosted LumaForge backend service.
- Change the Provisioner Worker HTTP API, job lifecycle, or runtime behavior.
- Change RunPod provisioning to consume a newly published image automatically.
- Add multi-worker orchestration or blue/green remote runtime rollout logic.
- Publish frontend or Tauri application artifacts.

## Decisions

1. **Use GitHub Actions as the deployment workflow.**

   The workflow will live under `.github/workflows/` and run from Git refs. This keeps deployment tied to code review, repository history, and GitHub-managed secrets.

   Alternative considered: a local deploy script. That would be simpler to write, but it would preserve the current problem of deployment depending on an operator's machine state and local credentials.

2. **Publish container images, not remote provider resources.**

   The deploy unit is the worker container image built from `workers/provisioner/Dockerfile`. Provider resource creation remains a native provisioning concern and should not be triggered by CI.

   Alternative considered: have the workflow create or update RunPod resources directly. That would couple CI to user-owned GPU infrastructure and introduce provider credentials into repository automation before the app has a stable production deployment model.

3. **Default to GitHub Container Registry with configurable image coordinates.**

   The workflow should be able to publish to GHCR using GitHub Actions credentials for the common case, while keeping the image name and registry overrideable through repository variables or workflow inputs. External registries can be supported by supplying explicit registry credentials as repository secrets.

   Alternative considered: hard-code one registry and image name. That reduces workflow inputs but makes the deployment artifact harder to move between private testing and release environments.

4. **Gate publishing behind worker validation.**

   The workflow will run the worker unit tests and a Docker build before pushing. Optional container smoke tests can run when the runner has Docker available and the required opt-in environment is set.

   Alternative considered: push after a successful Docker build only. That would miss Python-level regressions that are already covered by the worker test suite.

5. **Use deterministic tagging.**

   Every published image will receive a commit SHA tag. Release tag triggers can additionally publish a version tag. Manual dispatch can publish an operator-provided channel tag such as `staging` only after the same validation path succeeds.

   Alternative considered: publish only `latest`. That makes rollbacks and incident investigation ambiguous because the tag does not identify the source commit.

## Risks / Trade-offs

- Registry credentials can be misconfigured -> Document required secrets and fail before publishing if required values are absent.
- Channel tags such as `staging` can be overwritten -> Always publish the immutable SHA tag alongside mutable channel tags so rollback has a stable target.
- Docker cache behavior can hide build assumptions -> Keep tests outside the image build and use the Dockerfile as the single production packaging entry point.
- GHCR defaults may not match the eventual production registry -> Keep registry and image coordinates configurable without changing the workflow.

## Migration Plan

1. Add the GitHub Actions workflow with validation, build, login, metadata/tag generation, and push steps.
2. Add deployment documentation for triggers, required secrets/variables, produced tags, and rollback.
3. Run the workflow manually against a non-production channel to verify registry permissions.
4. Use the immutable SHA tag in downstream provisioning configuration only after the image has been verified.
5. Roll back by pointing downstream configuration to a previous SHA tag; mutable channel tags can be moved by re-running the workflow for the known-good ref.

## Open Questions

- Which production registry and image namespace will be used long term: GHCR under the repository owner, or an external registry managed outside GitHub?
