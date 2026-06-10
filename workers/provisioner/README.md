# LumaForge Provisioner Worker

Container-side worker that prepares a mounted ComfyUI workspace after the native layer calls `POST /start`.

## Local Run

```bash
cd workers/provisioner
LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
LUMA_FORGE_PROVISIONER_JOB_ID=workspace-id \
LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY=false \
LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  PYTHONPATH=src python -m app
```

The worker requires `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` before startup, listens on `127.0.0.1:8000` by default, and starts idle. It does not prepare the workspace until `/start` receives a selected Workflow Preset payload.

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
docker build -t luma-forge-provisioner:local -f Dockerfile ../..
docker run --rm \
  -e LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
  -e LUMA_FORGE_PROVISIONER_JOB_ID=workspace-id \
  -e LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY=false \
  -e LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  -p 8000:8000 \
  -v "$PWD/tmp-workspace:/workspace" \
  luma-forge-provisioner:local
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
| `LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY` | Required | Boolean text: `true` or `false`. |
| `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` | Required | JSON array of valid `ModelAsset` objects. |
| `LUMA_FORGE_PROVISIONER_HOST` | `127.0.0.1` | Valid IP address or DNS hostname. The container image sets `0.0.0.0`. |
| `LUMA_FORGE_PROVISIONER_PORT` | `8000` | Integer from `1` through `65535`. |
| `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` | `1048576` | Positive integer up to `104857600`. |
| `LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS` | `3600` | Positive finite number up to `86400`. |
| `LUMA_FORGE_WORKSPACE_MOUNT_PATH` | `/workspace` | Absolute normalized path. |

## API

Every endpoint requires:

```http
Authorization: Bearer <LUMA_FORGE_PROVISIONER_BEARER_TOKEN>
```

### `POST /start`

Starts one provisioning job. A second start while a job is active returns `409`.
The workspace mount path is read from `LUMA_FORGE_WORKSPACE_MOUNT_PATH` and defaults to `/workspace`.

```json
{
  "job_id": "workspace-id",
  "requires_hugging_face_api_key": false,
  "required_model_assets": [
    {
      "id": "model",
      "name": "Model",
      "download_source": {
        "source_type": "huggingface",
        "repository_id": "owner/model",
        "file_path": "model.safetensors",
        "revision": "main"
      },
      "install_comfyui_relative_path": "models/checkpoints/model.safetensors"
    }
  ]
}
```

Example:

```bash
curl -X POST http://127.0.0.1:8000/start \
  -H "Authorization: Bearer local-token-0123456789abcdef0123" \
  -H "Content-Type: application/json" \
  --data @start-request.json
```

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

Invalid requests return `400`:

```json
{
  "code": "invalid_request",
  "message": "job_id must be a non-empty string"
}
```

Calling `POST /start` while a job is active returns `409`:

```json
{
  "code": "active_job_exists",
  "message": "Provisioner worker already has an active job.",
  "context": {
    "active_job_id": "workspace-id"
  }
}
```

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

Oversized requests return `413`:

```json
{
  "code": "request_body_too_large",
  "message": "Request body is too large.",
  "context": {
    "max_request_bytes": 1048576
  }
}
```
