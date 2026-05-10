## 1. Catalog Validation

- [x] 1.1 Update bundled Workflow Catalog Git revisions for ComfyUI and Custom Nodes to immutable commit hashes.
- [x] 1.2 Keep model assets digest-free and update only Hugging Face download metadata needed by the worker.
- [x] 1.3 Add native validation for immutable worker-prepared Git revisions.
- [x] 1.4 Add native validation for Hugging Face model repository id, file path, revision, and install path.
- [x] 1.5 Update native catalog tests for immutable Git revision rejection and valid digest-free model assets.

## 2. Worker Configuration and API Hardening

- [x] 2.1 Add worker configuration for bearer token, maximum request body size, and external step timeouts.
- [x] 2.2 Require bearer-token authorization on all worker endpoints when configured.
- [x] 2.3 Reject missing, malformed, negative, or oversized `Content-Length` values before JSON parsing.
- [x] 2.4 Add API tests for authorized requests, unauthorized requests, disabled authorization, oversized bodies, and malformed body length.

## 3. Worker Error Taxonomy

- [x] 3.1 Add specific worker error classes and codes for authorization, request size, Git checkout, dependency install, asset download, asset auth, path validation, timeout, and cancellation.
- [x] 3.2 Map preparation failures to UI-safe API responses and job status error metadata without exposing command output or secrets.
- [x] 3.3 Update worker status and API tests to assert the new error codes.

## 4. Worker Preparation Hardening

- [x] 4.1 Validate ComfyUI and Custom Node Git revisions as immutable commits before clone, fetch, checkout, or dependency installation.
- [x] 4.2 Wrap Git commands and dependency installation with configured timeouts and cancellation handling.
- [x] 4.3 Replace ad hoc Hugging Face URL downloading with Hub-based public asset download behavior using repository id, file path, and revision.
- [x] 4.4 Rely on Hugging Face Hub caching for asset reuse without worker-owned asset validation.
- [x] 4.5 Map Hugging Face authentication failures to `asset_auth_required` and other download failures to `asset_download_failed`.
- [x] 4.6 Add preparer tests for immutable revision validation, timeout handling, Hub download invocation, Hub cache reuse behavior, and download error mapping.
- [x] 4.7 Install worker Python package dependencies in the production Docker image.

## 5. Verification

- [x] 5.1 Run `PYTHONPATH=src python3 -m unittest discover -s tests` in `workers/provisioner`.
- [x] 5.2 Run `cargo test` for native catalog validation changes.
- [x] 5.3 Run `cargo clippy --fix --allow-dirty --allow-staged` after Rust changes.
- [x] 5.4 Run `cargo fmt` after Rust changes.
- [x] 5.5 Regenerate and verify generated command bindings if native exported types changed.
- [x] 5.6 Run opt-in provisioner container smoke test and verify `huggingface_hub` is importable inside the image.
