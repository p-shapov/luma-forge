# LumaForge Provisioner Worker

Container-side one-shot worker that prepares a mounted ComfyUI workspace from startup environment configuration. The worker auto-starts the single provisioning job after configuration validation and exposes `GET /status` for authenticated progress polling.

## Local Run

```bash
cd workers/provisioner
LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
LUMA_FORGE_PROVISIONER_JOB_ID=workspace-id \
LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  PYTHONPATH=src python -m app
```

The worker requires `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`, `LUMA_FORGE_PROVISIONER_JOB_ID`, and `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` before startup. It listens on `127.0.0.1:8000` by default and auto-starts the single provisioning job as soon as the HTTP server is created. Clients observe progress with `GET /status`.

During preparation, the Provisioner Worker prepares only workspace-specific data on the mounted volume:

```text
/workspace/
  models/
  output/
  workflows/
```

The worker validates only workspace path safety and declared model asset files. It does not write `.luma-forge/runtime-manifest.json`, validate workflow or output paths, validate endpoint runtime paths, contain or validate the endpoint ComfyUI runtime, create the base virtual environment, clone ComfyUI, run `comfy install`, install ComfyUI base requirements, clone runtime extensions, or run `pip` during provisioning.

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
  -e LUMA_FORGE_PROVISIONER_JOB_ID=workspace-id \
  -e LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  -p 8000:8000 \
  -v "$PWD/tmp-workspace:/workspace" \
  provisioner:local
```

Optional smoke check:

```bash
cd workers/provisioner
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python -m unittest tests.test_container_smoke
```

Provisioner and endpoint images use separate Dockerfiles. The provisioner Dockerfile does not contain endpoint runtime stages or runtime contract build arguments.

## Deployment

See [Worker Deployment](../DEPLOYMENT.md) for image release triggers, registry conventions, catalog PR ownership, and rollback.

Remote provisioning must inject a unique per-pod bearer token through `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`, then send `Authorization: Bearer <token>` on every worker API request. The token must not be logged, persisted in workspace metadata, or returned in worker responses.

## Runtime Environment

The worker validates runtime environment before binding the HTTP server. Invalid configured values fail startup instead of falling back silently. Startup configuration failures write one JSON error record to stderr, exit with code `78`, and do not start the HTTP API:

```json
{
  "code": "invalid_integer",
  "env_name": "LUMA_FORGE_PROVISIONER_PORT",
  "message": "Invalid Provisioner Worker configuration for LUMA_FORGE_PROVISIONER_PORT: value must be an integer."
}
```

The error record never includes configured environment values or secrets.

| Variable | Default | Validation |
| --- | --- | --- |
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | Required | At least 32 ASCII characters; no whitespace or control characters. |
| `LUMA_FORGE_PROVISIONER_JOB_ID` | Required | Non-empty string. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | Required | JSON array of valid `ModelAsset` objects. |
| `LUMA_FORGE_PROVISIONER_HOST` | `127.0.0.1` | Valid IP address or DNS hostname. The container image sets `0.0.0.0`. |
| `LUMA_FORGE_PROVISIONER_PORT` | `8000` | Integer from `1` through `65535`. |
| `LUMA_FORGE_PROVISIONER_DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS` | `3600` | Positive finite number up to `86400`; measured since the last received download byte. |
| `LUMA_FORGE_WORKSPACE_MOUNT_PATH` | `/workspace` | Absolute normalized path. |
| `LUMA_FORGE_HUGGING_FACE_API_KEY` | Optional | Optional Hugging Face bearer token used for model downloads; never returned in worker responses. |

## API

Every endpoint requires:

```http
Authorization: Bearer <LUMA_FORGE_PROVISIONER_BEARER_TOKEN>
```

The worker API is observation-only. Provisioning starts from environment configuration at process startup. There is no start, retry, cancel, or termination endpoint. Native/control-plane code owns startup reachability timeout, status polling policy, cancellation policy, retry policy, and provider pod termination.

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
  "job_id": null,
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
  "job_id": "workspace-id",
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
  "job_id": "workspace-id",
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
