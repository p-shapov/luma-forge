## Context

Runtime recipe releases are the bridge between reviewed repository data and the worker images used by Workspace Provisioning. The current workflow validates recipe shape, builds provisioner and endpoint images, checks that both images declare the same runtime contract identity, publishes the image pair, and opens a Runtime Catalog PR.

Two gaps remain in that path. First, the recipe declares PyTorch index and packages, but the provisioner Docker build installs a hard-coded package set. Second, the workflow can append an implementation revision under an existing runtime contract id/version even when the selected recipe changes the contract compatibility surface. That undermines the meaning of persisted Workspace runtime snapshots.

## Goals / Non-Goals

**Goals:**

- Build the provisioner runtime archive from the recipe-declared PyTorch index URL and package list.
- Keep existing runtime contract versions immutable by rejecting incompatible recipe changes before image publication.
- Make the generated Runtime Catalog update derive runtime metadata from the selected recipe and verified image metadata.
- Add targeted regression coverage for the release transformation and guard behavior.

**Non-Goals:**

- Introduce a new runtime recipe schema.
- Change the current `.tar.gz` runtime archive format.
- Add support for multiple GPU cloud providers or non-RunPod endpoint images.
- Change native Workspace snapshot resolution behavior.

## Decisions

1. Keep recipe processing in the release workflow, but move non-trivial transformation into testable scripts if needed.

   The current logic is inline in `.github/workflows/deploy-runtime-recipe.yml`. Small extraction to a repository script is acceptable when it makes PyTorch argument generation and catalog compatibility checks unit-testable. The release workflow remains the orchestration boundary.

   Alternative considered: encode all checks directly in shell and inline Python. That minimizes files but makes regression coverage awkward and increases the chance of workflow-only drift.

2. Treat the runtime compatibility surface as normalized recipe metadata.

   Existing contract compatibility checks should compare the selected recipe against the existing catalog contract using normalized fields: Python version, platform, ComfyUI revision, PyTorch index URL, PyTorch package list, and base requirement list. The workflow should fail before publishing when an existing contract id/version differs on any normalized compatibility field.

   Alternative considered: compare only existing catalog `runtime_metadata`. That catches Python/platform/ComfyUI drift, but current catalog metadata does not include PyTorch packages or base requirements, so it would not protect the full recipe surface.

3. Pass PyTorch packages as explicit Docker build arguments.

   The workflow should serialize the recipe package list into a deterministic build argument and the Dockerfile should install exactly that list with the recipe index URL. The package list must preserve recipe order so lockstep package groups like `torch`, `torchvision`, and `torchaudio` remain operator-controlled.

   Alternative considered: generate a temporary requirements file during workflow execution and copy it into the Docker build context. That is more complex in GitHub Actions and adds transient file state where a build argument is sufficient.

4. Keep catalog image metadata path values tied to the built image contract.

   The catalog update should continue to use digest-pinned image refs and verified runtime metadata paths, but it should only be generated after the recipe-to-image checks pass. Existing implementation revisions remain immutable.

## Risks / Trade-offs

- Build arg quoting for package lists can break Docker installation if not normalized carefully -> Use a simple newline or JSON representation parsed by shell/Python in the Docker build, and add a regression test with multiple packages.
- Catalog compatibility comparison can miss fields not represented in the catalog -> Add or derive a runtime compatibility fingerprint/metadata field that captures recipe PyTorch and base requirement inputs.
- Failing before publish changes operator workflow for accidental recipe edits -> Error messages should name the mismatched fields and instruct operators to bump the runtime contract version.
- Extracting workflow logic into scripts adds another file boundary -> Keep scripts narrowly scoped and invoked by the workflow so local and CI behavior stay aligned.

## Migration Plan

1. Update the release workflow and Dockerfile so new releases use recipe-declared PyTorch settings.
2. Add compatibility validation before image publication and catalog PR generation.
3. Update catalog generation to preserve existing contracts only when compatibility metadata matches.
4. Add tests for recipe PyTorch build args and existing-contract mismatch rejection.
5. Roll back by reverting this change before publishing affected runtime images. After a published release, rollback remains selecting a previously valid implementation revision or shipping a corrected catalog/runtime contract version.

## Open Questions

- None. The compatibility fields are defined by the current recipe schema and runtime catalog invariants.
