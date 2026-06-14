# LumaForge Provisioner Worker

Container-side one-shot worker that prepares a mounted ComfyUI workspace from startup configuration. The worker auto-starts the single provisioning job after configuration validation and exposes `GET /status` for authenticated progress polling.

## Local Run

```bash
cd workers/provisioner
LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  PYTHONPATH=src python -m app
```

The worker requires `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` and `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` before startup. It listens on `0.0.0.0:8000` and auto-starts the single provisioning job as soon as the HTTP server is created. Clients observe progress with `GET /status`.

During preparation, the Provisioner Worker prepares only workspace-specific data on the mounted volume:

```text
/workspace/
  models/
  output/
  workflows/
```

## Test

```bash
cd workers/provisioner
PYTHONPATH=src python -m unittest discover -s tests
```

## Lint / Syntax Check

```bash
cd workers/provisioner
PYTHONPATH=src python -m compileall src tests
```

## Container

```bash
cd workers/provisioner
docker build -t provisioner:local -f Dockerfile ../..
docker run --rm \
  -e LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
  -e LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  -p 8000:8000 \
  -v "$PWD/tmp-workspace:/workspace" \
  provisioner:local
```

## Deployment

Publish the provisioner image by pushing a `provisioner-worker-v*` tag or running the `Deploy Provisioner` GitHub Actions workflow manually.

The workflow validates the provisioner package, builds `ghcr.io/<owner>/<repo>/provisioner-worker`, resolves the pushed digest, and opens a Runtime Contracts promotion PR. That PR appends the new provisioner contract revision to `bundled/runtime-contracts.json` and updates matching Workflow Presets in `bundled/workflow-catalog.json`.

New Workspaces use the image only after the promotion PR is reviewed, merged, and bundled into the app. Existing Workspaces remain pinned to their persisted provisioner image snapshot.

## Runtime Configuration

The worker validates startup environment before binding the HTTP server. Invalid configured values fail startup instead of falling back silently. Startup configuration failures write one JSON error record to stderr, exit with code `78`, and do not start the HTTP API:

```json
{
  "code": "invalid_json",
  "env_name": "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS",
  "message": "Invalid Provisioner Worker configuration for LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS: value must be valid JSON."
}
```

The error record never includes configured environment values or secrets.

| Variable | Default | Validation |
| --- | --- | --- |
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | Required | Non-empty string. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | Required | JSON array of valid `ModelAsset` objects. |
| `LUMA_FORGE_HUGGING_FACE_API_KEY` | Optional | Optional Hugging Face bearer token used for model downloads; never returned in worker responses. |

## API

Remote provisioning must inject a unique per-pod bearer token through `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`, then send `Authorization: Bearer <token>` on every worker API request. The token must not be logged, persisted in workspace metadata, or returned in worker responses.

The worker API is observation-only. Provisioning starts from environment configuration at process startup.

### `GET /status`

Returns the current worker status with structured error metadata when a job fails.

Example:

```bash
curl http://127.0.0.1:8000/status \
  -H "Authorization: Bearer local-token-0123456789abcdef0123"
```

```json
{
  "status": "idle",
  "phase": null,
  "progress_percent": null,
  "error": null,
  "updated_at": "2026-05-09T00:00:00Z",
  "provisioner_version": "1.0.0"
}
```

During an active job:

```json
{
  "status": "running",
  "phase": "downloading_assets",
  "progress_percent": 56,
  "error": null,
  "updated_at": "2026-05-09T00:00:00Z",
  "provisioner_version": "1.0.0"
}
```

Terminal states are `succeeded` and `failed`.

Failure responses include UI-safe error metadata:

```json
{
  "status": "failed",
  "phase": null,
  "progress_percent": 56,
  "error": {
    "code": "asset_download_failed",
    "message": "Hugging Face asset download failed"
  },
  "updated_at": "2026-05-09T00:00:00Z",
  "provisioner_version": "1.0.0"
}
```

## Error Responses

Worker API errors use `code` as the stable specific classifier. Safe structured metadata appears under `context` when available. Error payloads must not include `reason_code`, bearer tokens, provider API keys, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs.

Startup configuration failures are written to stderr before the HTTP API starts. The `env_name` field identifies the failed variable:

| Variable | Code | Meaning |
| --- | --- | --- |
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | `missing_required_value` | Bearer token environment variable is not set. |
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | `blank_value` | Bearer token is an empty string. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | `missing_required_value` | Required model assets environment variable is not set. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | `blank_value` | Required model assets value is empty after trimming whitespace. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | `invalid_json` | Required model assets value is not valid JSON. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | `invalid_request` | Required model assets value is valid JSON but does not match the required model asset contract. |

HTTP API errors are returned directly from worker endpoints:

| Code | HTTP status | Meaning |
| --- | ---: | --- |
| `invalid_authorization` | 401 | The `Authorization` header is missing, malformed, non-ASCII, or does not match the per-pod bearer token. |
| `endpoint_not_found` | 404 | The requested endpoint or HTTP method is unsupported. |
| `invalid_request` | 400 | The request shape is invalid. Reserved for request-taking endpoints. |
| `invalid_json` | 400 | The request body is not valid JSON. Reserved for request-taking endpoints. |
| `request_body_too_large` | 413 | The request body exceeds the worker limit. Reserved for request-taking endpoints. |
| `active_job_exists` | 409 | A new provisioning job was requested while another job was already running. |
| `worker_error` | 400 | Generic worker error fallback. |

Provisioning job failures appear under `error` in `GET /status` after the job reaches `status: "failed"`:

| Code | Meaning |
| --- | --- |
| `preparation_failed` | Workspace preparation or final model asset validation failed. |
| `asset_download_failed` | A required model asset could not be downloaded. |
| `asset_auth_required` | A Hugging Face asset requires authentication and no valid token was available. |
| `path_validation_failed` | A configured workspace-relative path is unsafe or escapes the workspace root. |
| `step_timeout` | A provisioning step timed out, including inactive model download streams. |
| `unexpected_exception` | An unhandled exception escaped the provisioning job. |

Unknown endpoints return `404`:

```json
{
  "code": "endpoint_not_found",
  "message": "Endpoint not found"
}
```

Unauthorized requests return `401`:

```json
{
  "code": "invalid_authorization",
  "message": "Unauthorized."
}
```
