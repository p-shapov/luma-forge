## 1. Invalid Start and Error Mapping Tests

- [x] 1.1 Add a test helper provisioner that records whether `prepare()` was called and can raise configured exceptions.
- [x] 1.2 Add invalid `POST /start` tests that assert the response is rejected, status remains `idle`, `prepare()` is not called, and a temporary workspace remains unchanged.
- [x] 1.3 Add unsafe preset invalid-start cases for unsafe identifiers, unsafe Custom Node paths, unsafe model asset paths, and mutable Git revisions.
- [x] 1.4 Add a table-driven terminal error mapping test for `GitCheckoutError`, `DependencyInstallError`, `AssetDownloadError`, `AssetAuthRequiredError`, `PathValidationError`, and `StepTimeoutError`.
- [x] 1.5 Keep or extend the unexpected exception sanitization test so sensitive-looking exception text and tracebacks are not exposed.

## 2. Path Escape Regression Tests

- [x] 2.1 Add `safe_child_path` tests for existing symlinks that point outside the workspace root.
- [x] 2.2 Add `safe_custom_node_child_path` tests for `custom_nodes` symlinks that resolve outside the prepared ComfyUI Custom Node root.
- [x] 2.3 Add prepared runtime path or validation tests for metadata, virtual environment, manifest, and model asset paths that would escape through symlinks.
- [x] 2.4 Use real temporary directories and skip only when the platform disallows symlink creation.

## 3. Real Provisioner Cancellation Tests

- [x] 3.1 Add fake command runner and downloader variants that honor `cancel_event` and expose deterministic wait/release events.
- [x] 3.2 Add `Provisioner.prepare()` cancellation tests before ComfyUI checkout, before dependency installation, before asset download, and before final validation.
- [x] 3.3 Add a cancellation-during-asset-placement test that verifies `.part` files are removed and the final model path is not promoted.
- [x] 3.4 Assert cancelled preparations do not write a success runtime manifest and do not run later phase collaborators.

## 4. Deployment Smoke Validation

- [x] 4.1 Update the provisioner container smoke test so CI can run it against a prebuilt image tag instead of always rebuilding.
- [x] 4.2 Update `.github/workflows/deploy-provisioner-worker.yml` to run the provisioner container smoke test after building the image and before GHCR login or publish.
- [x] 4.3 Update provisioner deployment documentation if the smoke test command or workflow behavior changes.

## 5. Verification

- [x] 5.1 Run `PYTHONPATH=src python3 -m unittest discover -s tests` from `workers/provisioner`.
- [x] 5.2 Run `PYTHONPATH=src python3 -m compileall src tests` from `workers/provisioner`.
- [x] 5.3 Run the provisioner container smoke test locally when Docker is available, or document why it was not run.
- [x] 5.4 Run `openspec status --change "harden-provisioner-worker-tests"` and confirm the change is apply-ready.
