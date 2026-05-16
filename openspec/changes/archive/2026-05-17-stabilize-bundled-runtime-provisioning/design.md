## Context

The completed bundled runtime implementation moved LumaForge toward runtime-contract-driven provisioning: Workflow Presets reference runtime contracts, Workspace Setup persists a resolved runtime implementation snapshot, the Provisioner Worker materializes a Docker-built ComfyUI runtime archive, and runtime recipe automation publishes matched provisioner/endpoint image pairs.

The current branch still has several contract gaps at the seams between native provisioning, RunPod responses, worker image metadata, runtime archive materialization, and endpoint manifest validation. Those gaps are severe because the runtime implementation snapshot is now authoritative: if any layer silently falls back to stale metadata or relative paths, provisioning can fail after provider resources have already been created.

## Goals / Non-Goals

**Goals:**

- Make the selected runtime implementation snapshot the only source of truth for provisioner and endpoint image identity during provisioning.
- Make provisioner image runtime identity explicit for the worker-side start contract.
- Ensure runtime materialization publishes all files that the prepared runtime manifest advertises.
- Ensure endpoint validation can resolve every manifest path under the mounted workspace.

**Non-Goals:**

- Do not introduce support for multiple GPU cloud providers.
- Do not change the high-level provisioning lifecycle or add new provider resources.
- Do not redesign the runtime catalog schema beyond the fields needed to stabilize the existing contract.
- Do not support prepared volumes created by arbitrary unreleased runtime archive layouts; this change targets newly materialized runtimes.

## Decisions

### Treat image identity as runtime implementation data

Native provisioning will create RunPod provisioning pods with the Workspace's persisted `resolved_runtime_implementation.provisioner_image_ref` and will also pass that same immutable ref into the pod environment as `LUMA_FORGE_PROVISIONER_IMAGE_REF`. The Provisioner Worker will compare the start request's resolved runtime implementation against image-exported runtime identity rather than a development placeholder fallback.

Alternative considered: keep the provisioner image ref as a code default. That is unsafe because real runtime catalog digests will never match the placeholder default and the worker cannot prove that it is the selected runtime implementation.

### Export provisioner runtime identity in the final image stage

The Dockerfile will declare the runtime contract build args in the final provisioner stage and export `LUMA_FORGE_RUNTIME_CONTRACT_ID`, `LUMA_FORGE_RUNTIME_CONTRACT_VERSION`, `LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION`, and `LUMA_FORGE_RUNTIME_ARCHIVE_PATH`. The publish workflow will provide `LUMA_FORGE_PROVISIONER_IMAGE_REF` at pod creation time after the immutable digest is known.

Alternative considered: bake the provisioner image ref into the image during Docker build. The final digest is not known until after push, so runtime pod configuration is the practical source for the immutable self-reference.

### Parse RunPod pod image identity defensively

RunPod pod responses will accept the documented `image` field and may continue accepting `imageName` for compatibility with existing tests or any legacy response variants. Mapping code will still require a non-empty image value before adopting or persisting a pod observation.

Native provider recovery will not compare provider-reported image identity against the resolved runtime implementation. Ownership identity for provisioning pod recovery remains provider name, volume, and placement. Runtime compatibility remains the Provisioner Worker's responsibility when Native starts the preparation job.

### Publish base runtime records alongside ComfyUI and the venv

Runtime archive extraction will stage `ComfyUI`, `.venv`, and `.luma-forge/base-runtime`. Materialization will publish the base runtime records into final workspace metadata paths before writing the runtime manifest. The manifest will contain absolute workspace-resolved paths for dependency records so endpoint validation does not resolve them relative to the endpoint process working directory.

Alternative considered: keep manifest dependency record paths relative and have endpoint validation join them against the workspace. That is viable but less explicit; using absolute workspace paths matches the existing manifest style for `python_path` and `comfyui_root`.

### Use an archive format the provisioner can extract

The build and materializer must agree on archive compression. The first implementation can either switch the runtime archive to a plain tar/gzip format supported by Python 3.12 `tarfile`, or keep `.tar.zst` and invoke the system `zstd` decompressor before passing tar bytes to `tarfile`. The selected implementation must be covered by a worker test using the same archive suffix and compression path used by Docker builds.

Alternative considered: rely on `tarfile.open(..., "r:*")` to infer zstd. Python 3.12 does not support zstandard tar archives through `tarfile`, so that path fails with the real Docker-built archive.

## Risks / Trade-offs

- Passing image refs through pod environment exposes non-secret deployment metadata -> Acceptable because image refs are not secrets and are already provider-visible.
- Switching archive compression may increase image size or build time -> Prefer correctness and Python 3.12 compatibility over compression ratio for v1.
- Accepting both `image` and `imageName` can hide provider drift -> Require a non-empty final image value and cover both variants with tests; update docs to treat `image` as canonical.

## Migration Plan

1. Add native/provider tests for RunPod `image` response parsing, provisioner pod env injection, and image-compatible pod adoption.
2. Update provisioner Docker/config/materializer/manifest behavior and worker tests for image-exported identity, archive extraction, published base records, and absolute manifest record paths.
3. Update endpoint validation tests for workspace-resolved dependency record paths.
4. Run native Rust verification, worker Python tests, and targeted Docker build validation for the runtime recipe path.

Rollback is reverting this change before publishing a release that depends on the runtime catalog. After release, rollback for new Workspaces is selecting a previously valid runtime implementation revision in the bundled catalog and shipping a catalog update; persisted Workspaces remain pinned to their resolved runtime implementation snapshots.

## Open Questions

- Should the first implementation prefer switching the archive to `.tar.gz` or keep `.tar.zst` with explicit `zstd` decompression? The design permits either as long as Docker build output and worker extraction tests use the same format.
