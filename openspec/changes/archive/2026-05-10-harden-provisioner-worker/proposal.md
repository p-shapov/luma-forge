## Why

The Provisioner Worker currently trusts its private container-local HTTP API, accepts unbounded request bodies, reports several operational failures through generic messages, and prepares mutable Git references. These gaps make provisioning harder to diagnose and leave avoidable hardening issues before the worker becomes part of the normal workspace provisioning path.

## What Changes

- Require optional bearer-token authorization for the worker HTTP API when a token is configured.
- Reject malformed or oversized request bodies before JSON parsing.
- Add specific UI-safe error codes for Git checkout, dependency installation, model download/authentication, path validation, timeout, cancellation, and authorization failures.
- Require bundled Git sources for ComfyUI and Custom Nodes to use immutable commit revisions instead of mutable branch names.
- Use Hugging Face Hub download semantics and caching for public model assets based on repository id, file path, revision, and install path; do not add extra app-owned asset metadata.
- Add bounded execution timeouts around Git operations, dependency installation, and model downloads.
- Keep secrets out of worker responses, diagnostics, logs, and persisted model metadata.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `provisioner-worker`: Harden the worker HTTP API, execution boundaries, model download behavior, and error taxonomy.
- `workspace-setup`: Tighten bundled workflow catalog validation for immutable Git revisions and public Hugging Face model download metadata.

## Impact

- `workers/provisioner`: HTTP handler, request parsing, preparation workflow, downloader, error mapping, configuration, and tests.
- `bundled/workflow-catalog.json`: Bundled source revisions and model file metadata.
- `src-tauri/src/domain/workflow`: Catalog validation rules and tests.
- Generated frontend command bindings may need regeneration if native type exports change, but this change does not add model digest metadata.
