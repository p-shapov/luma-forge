## Context

The provisioner worker currently prepares a mounted workspace volume from a selected Workflow Preset. The preparation path is implemented mostly in `preparer.py`, which now contains orchestration, subprocess execution, Git checkout, virtual environment creation, dependency installation, dependency record writing, Hugging Face download isolation, file placement, cancellation polling, timeout handling, and prepared environment validation.

The external worker contract is already covered by the `provisioner-worker` capability: the HTTP API, job lifecycle, progress reporting, cancellation, error mapping, dependency installation into the volume-local virtual environment, public Hugging Face model downloads, runtime manifest writing, and final validation behavior must remain unchanged.

## Goals / Non-Goals

**Goals:**

- Make `preparer.py` a readable orchestration module for the preparation workflow.
- Move low-level implementation responsibilities into focused modules with clear names and narrow tests.
- Preserve existing request/response behavior, job status transitions, progress phases, error codes, timeout behavior, cancellation behavior, prepared filesystem outputs, and runtime manifest behavior.
- Keep dependency injection for command execution and asset downloading so tests can continue to avoid real Git, pip, and network work.
- Keep the provisioner worker dependency profile unchanged.

**Non-Goals:**

- Do not change the worker HTTP API.
- Do not change the Workflow Preset schema or validation rules.
- Do not introduce parallel downloads, parallel Custom Node installation, retry policies, digest verification, private Hugging Face authentication, or new progress phases.
- Do not add a framework or service container.
- Do not change how the Endpoint Worker will consume the prepared runtime environment.

## Decisions

### Extract leaf responsibilities before changing orchestration shape

The first implementation step will move stable leaf responsibilities out of `preparer.py`: command execution, Hugging Face/public asset downloads, Git checkout, Python environment/dependency operations, and prepared environment validation. `Provisioner.prepare()` will keep the same high-level sequence while delegating to those modules.

Alternative considered: rewrite `Provisioner.prepare()` around a new step pipeline abstraction first. That would reduce visible method size, but it would also obscure behavior while moving many responsibilities at once. A leaf-first extraction keeps the review surface smaller and makes regressions easier to isolate.

### Keep explicit services instead of a generic step framework

The refactor will use focused services such as command execution, Git checkout, Python environment management, asset downloading, and environment validation. Each service should expose domain-specific methods rather than a generic `Step` interface.

Alternative considered: introduce a generic preparation step registry with names, progress percentages, and handlers. The current workflow is linear and small enough that a framework would add indirection without solving an active product need.

### Preserve existing import compatibility while moving code

Existing callers import `Provisioner` from `provisioner_worker.preparer`, and `JobManager` imports `Cancelled` from the same module. During the refactor, `preparer.py` should re-export moved public test seams where useful, or callers should be updated in the same change with compatibility considered explicitly.

Alternative considered: move all public classes and update every import immediately. That is clean long-term, but keeping a narrow compatibility layer reduces risk while tests are redistributed.

### Keep error mapping close to the failing subsystem

Git errors should remain mapped to `GitCheckoutError`, dependency and virtual environment failures to `DependencyInstallError`, public Hugging Face authorization failures to `AssetAuthRequiredError`, and public download failures to `AssetDownloadError`. The extracted modules should own those mappings instead of returning generic exceptions to the orchestrator.

Alternative considered: centralize all error mapping in `Provisioner.prepare()`. That would make the orchestration file larger again and force it to understand implementation-specific failure modes.

### Keep cancellation and timeout behavior shared

Subprocess cancellation and timeout behavior should live with command execution. Hugging Face download timeout isolation should live with the download module because it uses multiprocessing and queue result mapping rather than `subprocess.Popen`.

Alternative considered: create one global cancellation utility for both process types. The mechanics are different enough that sharing only small helper ideas is preferable to a common abstraction.

## Risks / Trade-offs

- Existing tests import classes from `preparer.py` -> keep re-exports temporarily or update tests and callers together.
- Splitting modules can create circular imports -> keep shared types in `schemas.py`, config in `config.py`, and runtime path/manifest logic in `runtime.py`; extracted modules must not import `Provisioner`.
- Behavior-preserving refactors can still change diagnostics accidentally -> assert existing worker error classes and key failure scenarios in tests after extraction.
- More files can feel heavier than one file -> keep modules focused and avoid tiny one-function modules unless they isolate a real subsystem.

## Migration Plan

1. Add focused modules under `workers/provisioner/src/provisioner_worker/` and move one responsibility at a time.
2. Keep `Provisioner.prepare()` behavior and progress calls unchanged while replacing private method bodies with delegated services.
3. Move or split tests so command execution, downloads, dependency handling, and validation are covered at their new module boundaries.
4. Run `python -m unittest discover -s workers/provisioner/tests` or the project’s provisioner worker test command, then run any container smoke tests that remain practical locally.

Rollback is straightforward because this change does not alter persisted data, external APIs, or runtime manifest schema. Reverting the refactor restores the previous module layout.

## Open Questions

- None.
