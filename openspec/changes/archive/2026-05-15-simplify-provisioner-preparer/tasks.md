## 1. Baseline And Boundaries

- [x] 1.1 Run the provisioner worker test suite to capture the current baseline before refactoring.
- [x] 1.2 Identify current external imports of `Provisioner`, `CommandRunner`, `PublicFileDownloader`, and `Cancelled`.
- [x] 1.3 Decide which moved classes remain re-exported from `preparer.py` for compatibility during this change.

## 2. Command Execution Extraction

- [x] 2.1 Move `CommandRunner` and subprocess timeout/cancellation behavior into a focused command execution module.
- [x] 2.2 Preserve `Cancelled` handling for `JobManager` and command execution callers.
- [x] 2.3 Move command runner tests to cover cancellation, timeout, startup failure, non-zero exit mapping, and large stdout capture at the new boundary.

## 3. Download Extraction

- [x] 3.1 Move `PublicFileDownloader`, `HubDownload`, Hugging Face client loading, isolated download process handling, auth-error detection, and file placement into a focused download module.
- [x] 3.2 Preserve public Hugging Face cache semantics, timeout termination, cancellation handling, auth failure mapping, and generic download failure mapping.
- [x] 3.3 Move download tests to cover direct download, cache reuse, auth failure, generic failure, and isolated-process timeout at the new boundary.

## 4. Preparation Service Extraction

- [x] 4.1 Move Git clone, fetch, and checkout behavior into a focused Git checkout helper or service.
- [x] 4.2 Move volume-local virtual environment creation, pip requirement installation, dependency report path generation, dependency record writing, and Python version capture into a focused Python environment helper or service.
- [x] 4.3 Move final prepared environment validation into a focused validator module.
- [x] 4.4 Keep existing error classes and timeout configuration use unchanged across all extracted services.

## 5. Orchestration Cleanup

- [x] 5.1 Update `Provisioner.prepare()` so it reads as the high-level preparation sequence and delegates implementation details to extracted modules.
- [x] 5.2 Introduce a small preparation context only if it materially reduces repeated path, progress, and cancellation argument passing without hiding the workflow order.
- [x] 5.3 Keep progress phases, progress percentages, diagnostic messages, manifest write ordering, and final validation ordering unchanged.

## 6. Verification

- [x] 6.1 Run the provisioner worker test suite after each major extraction or at minimum after the full refactor.
- [x] 6.2 Run formatting or linting commands used by the provisioner worker Python project, if configured.
- [x] 6.3 Confirm no new runtime dependencies were added.
- [x] 6.4 Confirm `preparer.py` no longer owns subprocess execution, Hugging Face download internals, dependency report path generation, or final validation internals.
