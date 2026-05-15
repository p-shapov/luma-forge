## 1. Baseline

- [x] 1.1 Run the provisioner worker test suite to capture the current baseline before moving modules.
- [x] 1.2 List current source and test imports that reference modules planned for relocation.

## 2. Package Structure

- [x] 2.1 Create `app/`, `api/`, `orchestration/`, `runtime/`, and `auxiliary/` packages directly under `workers/provisioner/src/`.
- [x] 2.2 Move startup wiring, config, worker error taxonomy, and request/schema parsing into `app/`.
- [x] 2.3 Move HTTP adapter code into top-level `api/`.
- [x] 2.4 Move job lifecycle management and high-level preparation sequencing into top-level `orchestration/`.
- [x] 2.5 Move Git checkout, Hugging Face model retrieval, path containment helpers, and generic process execution into top-level `auxiliary/`.
- [x] 2.6 Move runtime paths, manifest handling, Python environment management, dependency records, and prepared environment validation into top-level `runtime/`.

## 3. Import Migration

- [x] 3.1 Update production imports to use canonical subpackage module paths.
- [x] 3.2 Update test imports to use canonical subpackage module paths.
- [x] 3.3 Update Docker, README, and mock patch paths away from `provisioner_worker`.
- [x] 3.4 Remove or avoid `provisioner_worker` compatibility shims unless a real external boundary requires one.

## 4. Verification

- [x] 4.1 Run the provisioner worker test suite after the package move.
- [x] 4.2 Confirm package discovery still includes the new subpackages.
- [x] 4.3 Confirm no new runtime dependencies were added.
- [x] 4.4 Confirm `workers/provisioner/src/provisioner_worker/` no longer contains runtime implementation modules.
