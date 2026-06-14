# Provisioner Download Inactivity Timeout Design

## Context

The Provisioner Worker prepares a mounted ComfyUI workspace inside a provider-managed pod. Current code already auto-starts provisioning from validated environment configuration when the HTTP server is created, and the native layer polls `GET /status` for progress. The README is stale: it still describes an idle worker and a `POST /start` API.

The current download timeout setting is also misleading. `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS` is enforced as a total wall-clock deadline around the whole download process. Large downloads that are actively receiving bytes can therefore fail even when healthy.

The worker also carries two stale environment contract fields:

- `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY` duplicates behavior that can be handled by using `LUMA_FORGE_HUGGING_FACE_API_KEY` when present and mapping auth failures when absent.
- `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` is dead API configuration because the worker no longer accepts a JSON request body endpoint.

## Goals

- Document the provisioner as one-shot, non-retryable, and non-cancellable.
- Document env-driven auto-start and status polling through `GET /status`.
- State that native/control-plane owns startup reachability timeout, retry policy, cancellation policy, and provider pod termination.
- Replace the download wall-clock timeout with an inactivity timeout.
- Rename config, env, and function/variable names so they say inactivity.
- Remove `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY` from the worker contract.
- Document `LUMA_FORGE_HUGGING_FACE_API_KEY` as the optional Hugging Face secret env.
- Remove `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` from the worker contract.
- Keep changes scoped to provisioner config, schemas, downloader, narrow dead request-body handler cleanup, tests, and README.
- Verify with provisioner unit tests and provisioner compile check.

## Non-Goals

- No preparation wall-clock timeout.
- No worker cancellation API.
- No retry or resume manifest work.
- No native config injection change.
- No Dockerfile cleanup.
- No host or port configuration cleanup.
- No API lifecycle changes beyond deleting dead request-body handling.
- No broader lifecycle refactor.
- No compatibility shim for removed or renamed env vars.

## Contract

The Provisioner Worker is a one-shot workspace preparation process.

At startup, the worker validates environment configuration, creates the HTTP server, and immediately starts the single provisioning job from env-provided job metadata and model assets. Clients do not start jobs through the API. Clients authenticate with the configured bearer token and poll:

```http
GET /status
Authorization: Bearer <LUMA_FORGE_PROVISIONER_BEARER_TOKEN>
```

Terminal states remain `succeeded` and `failed`. The worker exposes no retry endpoint, no cancel endpoint, no start endpoint, and no pod termination endpoint. Native/control-plane code owns startup reachability timeout, status polling policy, cancellation policy, retry policy, and provider pod termination.

The env-derived job contract contains:

- `LUMA_FORGE_PROVISIONER_JOB_ID`
- `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS`

It no longer contains `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY`.

## Configuration

Rename download timeout configuration to inactivity-specific names:

- Env: `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS`
- Constant: `DEFAULT_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS`
- Config field: `download_inactivity_timeout_seconds`
- Downloader parameter: `download_inactivity_timeout_seconds`

The default remains `3600.0`. Validation remains a positive finite number up to `86400`.

Because LumaForge is pre-v1, the old env name is removed directly. The worker will not read, alias, warn on, or test compatibility with `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS`.

Remove `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY` from config parsing and schema parsing. The provisioner will pass `LUMA_FORGE_HUGGING_FACE_API_KEY` to Hugging Face downloads when it is configured. If the secret is absent, downloads proceed unauthenticated; gated or private assets fail through the existing auth-error mapping as `asset_auth_required`.

Remove `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` from worker config and README. The current contract exposes no JSON request body endpoint because provisioning auto-starts from env and clients only call `GET /status`. If removing the config field leaves unused request-body parsing in the API handler, delete that dead parsing path as part of this change.

Keep host and port config unchanged in this refactor.

## Downloader Design

The no-inactivity-timeout path can continue using direct synchronous download behavior for tests and internal callers that pass `None`.

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

## Tests

Update existing tests and helpers to the new inactivity names.

Downloader tests:

- Existing successful download still passes.
- Slow but continuously producing chunks does not time out.
- A stream that stops producing bytes triggers inactivity timeout.
- Inactivity timeout removes `.part` and does not leave the final target.
- Auth and generic download failure mapping remains unchanged.

Config and schema tests:

- Default inactivity timeout is used when env is absent.
- Explicit `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS` is accepted.
- Invalid inactivity timeout values are rejected.
- `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY` is removed from config and env-derived job schema.
- `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` is removed from worker config.
- Optional `LUMA_FORGE_HUGGING_FACE_API_KEY` parsing remains covered without leaking the value.

Documentation test or targeted README assertion:

- README describes env-driven auto-start and `GET /status`.
- README does not describe `POST /start`.
- README documents `LUMA_FORGE_HUGGING_FACE_API_KEY` as the optional Hugging Face secret.
- README does not list `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY`.
- README does not list `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES`.
- README states that retry, cancellation, startup reachability timeout, and pod termination are not worker API responsibilities.

## Validation

Run from repository root:

```bash
PYTHONPATH=workers/provisioner/src python3 -m unittest discover -s workers/provisioner/tests
PYTHONPATH=workers/provisioner/src python3 -m compileall workers/provisioner/src workers/provisioner/tests
```

No native tests are required unless implementation unexpectedly touches native code.
