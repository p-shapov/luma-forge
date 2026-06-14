# Provisioner Download Inactivity Timeout Design

## Context

The Provisioner Worker prepares a mounted ComfyUI workspace inside a provider-managed pod. Current code already auto-starts provisioning from validated environment configuration when the HTTP server is created, and the native layer polls `GET /status` for progress. The README is stale: it still describes a `/start` API and an idle startup contract.

The current download timeout setting is also misleading. `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS` is enforced as a total wall-clock deadline around the whole download process. Large downloads that are actively receiving bytes can therefore fail even when healthy.

## Goals

- Document the provisioner as one-shot, non-retryable, and non-cancellable.
- Document env-driven auto-start and status polling through `GET /status`.
- State that native/control-plane owns pod termination and startup reachability timeout.
- Replace the download wall-clock timeout with an inactivity timeout.
- Rename config, env, and function/variable names so they say inactivity.
- Keep the change scoped to provisioner config, downloader, request handler cleanup, tests, README, and Dockerfile env cleanup.
- Verify with provisioner unit tests and provisioner compile check.

## Non-Goals

- No preparation wall-clock timeout.
- No worker cancellation API.
- No retry or resume manifest work.
- No native config injection change.
- No broader lifecycle refactor.
- No compatibility shim for `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS`.

## Contract

The Provisioner Worker is a one-shot workspace preparation process.

At startup, the worker validates environment configuration, creates the HTTP server, and immediately starts the single provisioning job from env-provided job metadata and model assets. Clients do not start jobs through the API. Clients authenticate with the configured bearer token and poll:

```http
GET /status
Authorization: Bearer <LUMA_FORGE_PROVISIONER_BEARER_TOKEN>
```

Terminal states remain `succeeded` and `failed`. The worker exposes no retry endpoint, no cancel endpoint, and no pod termination endpoint. Native/control-plane code owns startup reachability timeout, status polling policy, and provider pod termination.

## Configuration

Rename download timeout configuration to inactivity-specific names:

- Env: `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS`
- Constant: `DEFAULT_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS`
- Config field: `download_inactivity_timeout_seconds`
- Downloader parameter: `download_inactivity_timeout_seconds`

The default remains `3600.0`. Validation remains a positive finite number up to `86400`.

Because LumaForge is pre-v1, the old env name is removed directly. The worker will not read, alias, warn on, or test compatibility with `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS`.

Remove host and port runtime configuration from the worker:

- Delete `LUMA_FORGE_PROVISIONER_HOST` parsing from worker config.
- Delete `LUMA_FORGE_PROVISIONER_PORT` parsing from worker config.
- Replace them with internal worker constants: bind host `0.0.0.0`, port `8000`.

This avoids fake flexibility. Native already constructs the provisioner status URL with port `8000`, and the provider container must bind outside loopback to be reachable through Docker port publishing and RunPod proxying. Making host configurable only adds a way to create an unreachable pod.

Remove `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` from worker config and README. The current contract exposes no JSON request body endpoint because provisioning auto-starts from env and clients only call `GET /status`. Keeping request body size configuration after removing `/start` would document dead behavior.

Document `LUMA_FORGE_HUGGING_FACE_API_KEY` in the README as an optional secret env. It is required only when `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY=true`; the worker must not echo, log, or expose the raw value.

## Downloader Design

The no-timeout path can continue using direct synchronous download behavior for tests and internal callers that pass `None`.

The configured inactivity timeout path keeps the existing child-process isolation. The child process resolves the Hugging Face URL, opens the request, creates the `.part` file, and reads response chunks manually instead of using `copyfileobj`.

For each successful non-empty chunk:

1. Write the chunk to the `.part` file.
2. Send a small progress message to the parent process.
3. Continue reading.

The parent process owns inactivity tracking:

1. Initialize `last_chunk_at` when the child process starts.
2. Drain progress messages while the child is alive.
3. Update `last_chunk_at` after each progress message.
4. If `monotonic() - last_chunk_at` exceeds the configured inactivity threshold, terminate the child process.
5. Remove the `.part` file if it exists.
6. Raise `StepTimeoutError`.

When the child exits normally, the parent reads the final status message. Auth errors continue to map to `AssetAuthRequiredError`; other failures continue to map to `AssetDownloadError`.

Large downloads that continuously produce non-empty chunks must never fail because of total elapsed time. A stream that blocks before the first chunk or stops after a partial chunk fails after the inactivity window.

## Dockerfile Env Audit And Cleanup

`workers/provisioner/Dockerfile` currently sets:

- `PYTHONUNBUFFERED=1`
- `LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace`
- `LUMA_FORGE_PROVISIONER_HOST=0.0.0.0`
- `LUMA_FORGE_PROVISIONER_PORT=8000`

Keep `PYTHONUNBUFFERED=1`; it controls Python output buffering.

Remove `LUMA_FORGE_PROVISIONER_HOST=0.0.0.0` and `LUMA_FORGE_PROVISIONER_PORT=8000`; worker code will own those fixed bind values.

Remove `LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace`; it duplicates the worker default and native mount-path assumption.

Do not add `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS` to the Dockerfile. The worker default remains authoritative.

## Tests

Update existing tests and helpers to the new inactivity names.

Downloader tests:

- Existing successful download still passes.
- Slow but continuously producing chunks does not time out.
- A stream that stops producing bytes triggers inactivity timeout.
- Inactivity timeout removes `.part` and does not leave the final target.
- Auth and generic download failure mapping remains unchanged.

Config tests:

- Default inactivity timeout is used when env is absent.
- Explicit `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS` is accepted.
- Invalid inactivity timeout values are rejected.
- Host and port are no longer accepted as worker config fields or env-driven options.
- Max request bytes is no longer accepted as a worker config field or env-driven option.
- Optional Hugging Face API key parsing remains covered without leaking the value.

Documentation test:

- README describes env-driven auto-start and `GET /status`.
- README documents fixed worker bind host `0.0.0.0` and port `8000` as implementation/runtime constants, not env variables.
- README does not list `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES`.
- README lists `LUMA_FORGE_HUGGING_FACE_API_KEY` as the optional Hugging Face secret.
- README states that retry/cancel/termination are not worker API responsibilities.

## Validation

Run from repository root:

```bash
PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests
PYTHONPATH=workers/provisioner/src python3 -m compileall workers/provisioner/src workers/provisioner/tests
```

No native tests are required unless implementation unexpectedly touches native code.
