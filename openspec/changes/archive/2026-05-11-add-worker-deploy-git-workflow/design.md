## Context

The Provisioner Worker is a Python service packaged by `workers/provisioner/Dockerfile`. The repository documents local Docker build and smoke-test commands, but it does not yet provide a GitHub Actions workflow that turns a reviewed Git ref into a published worker image. LumaForge v1 has no hosted backend; deployment for this component means publishing a verified container image that remote GPU provisioning can reference later.

The workflow must fit the current repository layout, keep secrets in GitHub Actions, and avoid changing the worker HTTP API or the native provisioning lifecycle as part of this change.

## Goals / Non-Goals

**Goals:**

- Add a GitHub Actions workflow that validates, builds, tags, and publishes the Provisioner Worker image.
- Support automatic deployment from release tags and controlled manual deployment through `workflow_dispatch` without manual inputs.
- Use immutable commit SHA image tags for every published image.
- Publish to GitHub Container Registry through GitHub Actions token context.
- Document the triggers, image path, produced tags, and rollback process required to operate the workflow.

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

3. **Use one GHCR image path.**

   The workflow publishes to `ghcr.io/<owner>/<repo>/provisioner-worker` using GitHub Actions token context. This removes manual registry choices from the operator path and keeps the action form focused on selecting the ref.

   Alternative considered: make the registry and image path configurable. That supports more deployment targets, but it creates form fields and branches that are not needed for the current deployment target.

4. **Gate publishing behind worker validation.**

   The workflow will run the worker unit tests and a Docker build before pushing. Optional container smoke tests can run when the runner has Docker available and the required opt-in environment is set.

   Alternative considered: push after a successful Docker build only. That would miss Python-level regressions that are already covered by the worker test suite.

5. **Use deterministic tagging.**

   Every published image will receive a commit SHA tag. Release tag triggers also publish the release tag. Manual dispatch publishes only the immutable SHA tag for the selected ref.

   Alternative considered: publish only `latest`. That makes rollbacks and incident investigation ambiguous because the tag does not identify the source commit.

## Risks / Trade-offs

- GHCR package permissions can be misconfigured -> Use minimal `packages: write` workflow permissions and document the fixed GHCR image path.
- Docker cache behavior can hide build assumptions -> Keep tests outside the image build and use the Dockerfile as the single production packaging entry point.
- A future external registry would require a workflow change -> Keep the current workflow simple until a real second registry exists.

## Migration Plan

1. Add the GitHub Actions workflow with validation, build, login, metadata/tag generation, and push steps.
2. Add deployment documentation for triggers, GHCR image path, produced tags, and rollback.
3. Run the workflow manually to verify GHCR package permissions.
4. Use the immutable SHA tag in downstream provisioning configuration only after the image has been verified.
5. Roll back by pointing downstream configuration to a previous SHA tag; mutable channel tags can be moved by re-running the workflow for the known-good ref.

## Open Questions

None.
