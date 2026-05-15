## Context

The provisioner worker started as a small package, so a flat module layout under `src/provisioner_worker/` was enough. Recent preparation refactors made responsibilities clearer, but they also added more root-level modules inside that package: `preparer.py`, `command_runner.py`, `downloads.py`, `git_checkout.py`, `python_environment.py`, and `environment_validator.py` now sit beside API, server, config, errors, schemas, paths, and runtime helpers.

The next issue is not individual module size; it is source-tree scanability. Because `workers/provisioner` contains a single worker application, the extra `provisioner_worker` package wrapper does not add much signal. Responsibility-based packages can live directly under `src/` as the worker’s application source layout.

## Goals / Non-Goals

**Goals:**

- Group provisioner worker modules by responsibility using a small number of top-level packages directly under `src/`.
- Remove the `provisioner_worker` package wrapper from runtime code.
- Keep worker launch wiring obvious after replacing `python -m provisioner_worker`.
- Keep preparation as the high-level runtime-preparation use case, not a bucket for every helper invoked by that use case.
- Make acquisition, prepared-runtime, filesystem safety, process execution, API transport, and job lifecycle boundaries explicit.
- Preserve runtime behavior, public worker API behavior, error mapping, progress reporting, and prepared filesystem outputs.
- Keep imports explicit and avoid compatibility shims unless they protect a real external boundary.

**Non-Goals:**

- Do not change the HTTP API, request schemas, response schemas, job state model, or runtime manifest shape.
- Do not add new runtime dependencies.
- Do not rewrite preparation logic or introduce new abstractions beyond package organization.
- Do not archive or remove OpenSpec changes as part of this implementation.

## Decisions

### Use top-level responsibility packages under `src/`

The worker source tree should be organized around the main reasons code changes:

```text
src/
  app/
    __init__.py
    __main__.py
    server.py
    config.py
    errors.py
    schemas.py
  api/
    __init__.py
    handler.py
  orchestration/
    __init__.py
    preparation_job.py
    preparer.py
  runtime/
    __init__.py
    python_environment.py
    manifest.py
    validation.py
  auxiliary/
    __init__.py
    command_runner.py
    git.py
    huggingface.py
    paths.py
```

Package ownership:

- `app/` owns process startup, dependency wiring, configuration, request parsing DTOs, and UI-safe worker error taxonomy. These are app-wide contracts, but they are small enough and central enough to keep inside `app/`.
- `api/` owns the HTTP adapter only: authorization, body parsing, route dispatch, and response serialization.
- `orchestration/` owns application-level long-running work: active-job exclusivity, worker thread lifecycle, cancellation state, progress snapshots, terminal job state mapping, and the high-level prepare-runtime sequence. It delegates runtime, auxiliary, filesystem, and process details.
- `runtime/` owns the prepared runtime environment contract: runtime paths, volume-local Python environment setup, dependency records, runtime manifest read/write, and final prepared-environment validation.
- `auxiliary/` owns support mechanisms used by the worker but not central application flows: Git repository checkout, Hugging Face model file retrieval/cache placement, path safety helpers, and low-level process execution/cancellation.

Alternative considered: place loose modules directly in `src/` beside `api/`, `preparation/`, and `infrastructure/`. That is closer to the user-facing idea of “directories directly in `src`,” but the current setuptools config only discovers packages under `src`; loose modules would require explicit `py_modules` configuration and would make imports such as `config` or `server` too generic. A small `app/` package keeps all runtime code package-discoverable while removing the redundant `provisioner_worker` wrapper.

Alternative considered: put `git_checkout.py`, `python_environment.py`, and `environment_validator.py` under `preparation/` because `Provisioner.prepare()` calls them. That is the wrong boundary: Git and Hugging Face are support concerns, Python environment/manifest/validation are prepared-runtime concerns, and the preparation package should remain the use-case orchestrator rather than owning every implementation detail.

Alternative considered: keep separate `acquisition/`, `filesystem/`, and `infrastructure/` top-level packages. Those names are accurate, but each package would be very small and would make support code look like three separate application boundaries. A single `auxiliary/` package keeps helper code visually grouped while module names preserve the specific responsibility.

Alternative considered: put `runtime` under `contracts/` because the Endpoint Worker will eventually consume the prepared runtime manifest. For now, the runtime module also owns filesystem paths and validation implementation used by the provisioner worker. If the manifest becomes a shared inter-worker contract later, split manifest DTOs into `contracts/` while leaving provisioner-specific runtime setup in `runtime/`.

### Prefer direct imports over root compatibility modules

Internal source and tests should import moved modules from their canonical package paths, such as `app.config`, `app.schemas`, `app.errors`, `api.handler`, `orchestration.preparation_job`, `orchestration.preparer`, `runtime.manifest`, `auxiliary.git`, `auxiliary.paths`, and `auxiliary.command_runner`. Compatibility shims for `provisioner_worker.*` should not be retained unless a real external boundary requires them.

Alternative considered: keep a `provisioner_worker` compatibility package that re-exports every moved file. That would reduce import churn, but it would preserve the wrapper this change is meant to remove.

### Update launch and packaging wiring

The container currently launches the provisioner worker with `python -m provisioner_worker`. After removing that package, launch wiring should use the new app package, likely `python -m app`. README commands, Docker CMD, tests, and any `unittest.mock.patch` paths must be updated accordingly.

Alternative considered: add a console script entry point and launch that command. This is a reasonable future cleanup, but `python -m app` is a smaller mechanical change that mirrors the current launch style.

### Move files mechanically first, then clean imports

Implementation should first move files into the target subpackages while preserving contents, then update imports and run tests. Behavior changes should be avoided so failures point to import or packaging issues, not logic changes.

Alternative considered: combine the move with another cleanup pass. That makes review harder and increases the chance of accidentally changing behavior.

## Risks / Trade-offs

- Import churn can cause breakage -> move mechanically, update tests, and run the full provisioner worker suite.
- Python package discovery can miss runtime modules -> put runtime code in packages with `__init__.py` files and keep setuptools package discovery unchanged.
- Generic top-level package names can collide in larger environments -> this worker container installs one worker application, and package names are only used internally; avoid looser names like root-level `config.py`.
- More packages can make a small worker feel heavier -> use shallow packages named by ownership, and do not add deeper nesting until a package needs it.
- Future shared prepared-runtime contracts may outgrow `runtime/manifest.py` -> treat this as current ownership, not a permanent shared-library boundary.

## Migration Plan

1. Create `app/`, `api/`, `orchestration/`, `runtime/`, and `auxiliary/` packages directly under `workers/provisioner/src/`.
2. Move modules into the target packages.
3. Update all source and test imports to canonical paths.
4. Update Docker and README launch commands from `python -m provisioner_worker` to the new app package.
5. Run the provisioner worker test suite with the same `PYTHONPATH` used by local tests.
6. Confirm no `src/provisioner_worker/` runtime package remains.

## Open Questions

- None.
