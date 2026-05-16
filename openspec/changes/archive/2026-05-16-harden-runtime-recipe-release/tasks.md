## 1. Release Metadata Preparation

- [x] 1.1 Extract or refactor runtime recipe parsing so recipe runtime metadata can be reused by workflow validation, Docker build arguments, and catalog generation.
- [x] 1.2 Normalize recipe compatibility metadata including Python version, platform, ComfyUI revision, PyTorch index URL, PyTorch package list, base requirements, and runtime manifest compatibility fields.
- [x] 1.3 Add regression coverage for normalized compatibility metadata generation from `workers/runtime-recipes/comfyui-python312-cu121.yaml`.

## 2. PyTorch Build Wiring

- [x] 2.1 Export recipe-declared PyTorch index URL and package list from the runtime recipe resolution step.
- [x] 2.2 Add provisioner Docker build arguments for the PyTorch index URL and package list.
- [x] 2.3 Update `workers/Dockerfile` so the runtime builder installs the recipe-provided PyTorch packages instead of hard-coded package versions.
- [x] 2.4 Add a regression test or workflow tooling test proving the Docker build receives the recipe PyTorch package set.

## 3. Contract Compatibility Guard

- [x] 3.1 Before publishing images, load `bundled/runtime-catalog.json` and detect whether the selected recipe targets an existing runtime contract id/version.
- [x] 3.2 Reject existing-contract releases when normalized recipe compatibility metadata differs from the catalog contract compatibility metadata.
- [x] 3.3 Ensure the failure message names the mismatched fields and instructs operators to bump the runtime contract version or restore the recipe.
- [x] 3.4 Add regression coverage for compatible existing-contract append and incompatible existing-contract rejection.

## 4. Catalog Update Alignment

- [x] 4.1 Generate new Runtime Catalog contract metadata from normalized recipe metadata when the contract id/version is new.
- [x] 4.2 Preserve existing implementation revisions unchanged when appending a compatible implementation revision.
- [x] 4.3 Keep generated implementation image refs digest-pinned and image metadata paths aligned with the built provisioner and endpoint images.

## 5. Verification

- [x] 5.1 Run the release tooling regression tests added for this change.
- [x] 5.2 Run Provisioner Worker tests.
- [x] 5.3 Run RunPod Endpoint Worker tests.
- [x] 5.4 Run a targeted workflow or local dry-run validation for the runtime recipe release path without publishing images.
