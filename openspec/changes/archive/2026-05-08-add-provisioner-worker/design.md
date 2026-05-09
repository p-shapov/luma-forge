## Context

The existing Workspace Setup flow persists `Draft` Workspaces and bundled catalog snapshots, while the Workspace Provisioning flow is documented but not implemented. Provisioning Profiles already reference a `ghcr.io/luma-forge/provisioner:1.0.0` image and status endpoint, but there is no worker implementation or HTTP contract for preparing a mounted ComfyUI environment.

This change introduces the container-side Provisioner Worker only. It lives under `/workers/provisioner`, runs inside a temporary Provisioning Pod, receives a selected Workflow Preset through `/start`, prepares the mounted volume, and reports progress. Native RunPod resource orchestration remains outside scope.

## Goals / Non-Goals

**Goals:**

- Add a Python HTTP worker runtime with `POST /start`, `POST /cancel`, and `GET /status`.
- Package the worker as a container image that exposes the Provisioning Profile status port.
- Prepare a mounted workspace volume by installing ComfyUI, optional Custom Nodes, and public Hugging Face assets from the selected Workflow Preset.
- Make provisioning start explicit through `/start`; the worker must not begin preparation on container boot.
- Reject concurrent `/start` requests while a job is active.
- Keep worker status UI-safe and secret-free.
- Make model asset target paths explicit in bundled Workflow Presets.

**Non-Goals:**

- No native Workspace Provisioning orchestrator, RunPod pod creation, provider resource mutation, or Tauri command wiring.
- No React UI for provisioning.
- No Endpoint Worker runtime.
- No Hugging Face API key support, private model support, or secret handling.
- No multi-job queue inside the worker.
- No durable worker database; the mounted workspace filesystem is the durable preparation target.

## Decisions

### Use a Python HTTP Service in `/workers/provisioner`

The worker should be owned as a separate runtime boundary under `/workers/provisioner` instead of being mixed into the Tauri app or frontend packages. A Python service can run naturally in the provisioner container, coordinate subprocess-based Git/pip/download operations, and be tested independently from Rust and React.

Alternative considered: implement the worker in Rust inside the native layer. This was rejected because the worker runs inside the remote Provisioning Pod and should not depend on the desktop app runtime.

### Start Work Only Through `POST /start`

The container should boot into an idle HTTP server and perform no environment preparation until `/start` receives a selected Workflow Preset payload. This gives the future native orchestrator an explicit synchronization point: create pod, wait for worker readiness, then start a correlated job.

Alternative considered: auto-start on container boot using environment variables. This was rejected because it makes retries, validation failures, and future cancellation harder to coordinate from native provisioning.

### Keep One Active Job Per Worker

The worker should support at most one active provisioning job. If `/start` is called while a job is active, it should return a conflict error instead of queueing or replacing the job. The Provisioning Pod is temporary and workspace-specific, so an in-container queue would add complexity without a v1 use case.

Alternative considered: allow idempotent duplicate `/start` for the same job. This was rejected for v1 because the future native orchestrator can treat a conflict plus `/status` as the safe recovery path while the worker keeps simpler concurrency semantics.

### Use Explicit Asset Install Paths from Workflow Presets

Model assets should declare an explicit ComfyUI-relative write path in the Workflow Preset instead of the worker inferring paths from `model_asset_kind`. The worker should validate that paths are relative, non-empty, normalized, and remain under the prepared ComfyUI root before writing.

Alternative considered: hardcode kind-to-directory mapping in the worker. This was rejected because model placement is catalog knowledge and will vary as presets become more complex.

### Treat Public Hugging Face as the Only v1 Model Source

The worker should download Hugging Face assets using public repository/file/revision data from the selected preset and must not accept or read Hugging Face API keys. This avoids secret handling inside the worker and matches the current v1 decision.

Alternative considered: add optional token support immediately. This was rejected because private/gated assets introduce logging, environment, filesystem, and API contract risks that are not needed for the first worker.

### Report Structured Status, Not Native Workspace State

The worker status response should expose job status, current phase, optional progress percentage, UI-safe diagnostic message, updated timestamp, and terminal error metadata. It should not expose provider resource state, Workspace lifecycle, Provider API Keys, Hugging Face tokens, or native cleanup decisions.

The future native orchestrator can map worker phases into `WorkspaceProvisioningProgress`, but the worker should remain unaware of Workspace Catalog persistence.

Alternative considered: make the worker return native `WorkspaceProvisioningProgress` directly. This was rejected because it would couple the worker to desktop-owned lifecycle semantics.

### Prefer Filesystem-Based Idempotency Within a Job

The worker should make individual preparation steps tolerant of already-present expected files and directories when validation passes. This lets retries within a running job avoid unnecessary rework. The worker should still return a terminal failure when existing state is inconsistent, unsafe, or cannot be validated.

Alternative considered: require an empty volume for every run. This was rejected because partial downloads or installs can happen after transient failures, and safe reuse reduces wasted provisioning time.

## Risks / Trade-offs

- Public Hugging Face files may change or disappear despite revisions -> The worker validates download failures and reports a terminal failure without fabricating success.
- Explicit write paths can become unsafe write primitives -> The worker rejects absolute paths, parent traversal, blank paths, and paths outside the ComfyUI root.
- Dependency installation may be slow or flaky -> Status phases and messages expose current progress while failures remain terminal and UI-safe.
- Cancellation cannot always stop subprocesses instantly -> Cancellation should be best-effort, transition through `cancelling`, and terminate as `cancelled` only after active work has stopped safely.
- No durable worker database means in-memory job state is lost on container restart -> The future native orchestrator remains responsible for pod lifecycle recovery; this worker only validates filesystem state during active execution.
- Container image contents can drift from catalog expectations -> The image tag/digest remains referenced by Provisioning Profiles, and the worker should expose a version in status for diagnostics.

## Migration Plan

There is no existing Provisioner Worker implementation to migrate.

Implementation should add the worker directory, its Python dependency metadata, tests, and container image definition without changing native provisioning behavior. The bundled Workflow Catalog must be migrated so every model asset includes an explicit ComfyUI-relative install path. Rollback before release can remove `/workers/provisioner` and restore the previous catalog shape because no supported native provisioning flow depends on it yet.

## Open Questions

None.
