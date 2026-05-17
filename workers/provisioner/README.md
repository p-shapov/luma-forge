# LumaForge Provisioner Worker

Container-side worker that prepares a mounted ComfyUI workspace after the native layer calls `POST /start`.

## Local Run

```bash
cd workers/provisioner
LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
  PYTHONPATH=src python -m app
```

The worker requires `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` before startup, listens on `127.0.0.1:8000` by default, and starts idle. It does not prepare the workspace until `/start` receives a selected Workflow Preset payload.

During preparation, the Provisioner Worker validates the image-baked ComfyUI base runtime under `/opt/luma-forge/runtime` and prepares only workspace-specific data on the mounted volume. Workflow Preset Custom Nodes, model assets, runtime metadata, and Custom Node dependency overlays live on `/workspace`:

```text
/workspace/
  custom_nodes/
  models/
  output/
  .luma-forge/
    runtime-manifest.json
    python-overlay/
```

The worker must not create the base virtual environment, clone ComfyUI, extract a base runtime archive, or install ComfyUI base requirements during provisioning. The endpoint worker later starts ComfyUI through the image-baked Python interpreter and adds workspace Custom Node, model, output, and overlay paths.

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
docker build -t luma-forge-provisioner:local -f ../Dockerfile --target provisioner ../..
docker run --rm \
  -e LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
  -p 8000:8000 \
  -v "$PWD/tmp-workspace:/workspace" \
  luma-forge-provisioner:local
```

Optional smoke check:

```bash
cd workers/provisioner
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python -m unittest tests.test_container_smoke
```

Provisioner and endpoint images are built from the shared provider-neutral Dockerfile at `workers/Dockerfile`.

## Deployment

See [DEPLOYMENT.md](./DEPLOYMENT.md) for the GitHub Actions worker image deployment workflow, required registry configuration, produced tags, and rollback process.

Remote provisioning must inject a unique per-pod bearer token through `LUMA_FORGE_PROVISIONER_BEARER_TOKEN`, then send `Authorization: Bearer <token>` on every worker API request. The token must not be logged, persisted in workspace metadata, or returned in worker responses.

## Runtime Environment

The worker validates runtime environment before binding the HTTP server. Invalid configured values fail startup instead of falling back silently. Startup configuration failures write one JSON diagnostic to stderr, exit with code `78`, and do not start the HTTP API:

```json
{
  "code": "configuration_error",
  "env_name": "LUMA_FORGE_PROVISIONER_PORT",
  "reason_code": "invalid_integer",
  "message": "Invalid Provisioner Worker configuration for LUMA_FORGE_PROVISIONER_PORT: value must be an integer."
}
```

The diagnostic never includes configured environment values or secrets.

| Variable | Default | Validation |
| --- | --- | --- |
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | Required | At least 32 ASCII characters; no whitespace or control characters. |
| `LUMA_FORGE_PROVISIONER_HOST` | `127.0.0.1` | Valid IP address or DNS hostname. The container image sets `0.0.0.0`. |
| `LUMA_FORGE_PROVISIONER_PORT` | `8000` | Integer from `1` through `65535`. |
| `LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES` | `1048576` | Positive integer up to `104857600`. |
| `LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS` | `1800` | Positive finite number up to `86400`. |
| `LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS` | `1800` | Positive finite number up to `86400`. |
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
  "workflow_preset": {
    "id": "comfyui-t2i-basic",
    "version": "1.0.0",
    "name": "ComfyUI Text to Image Basic",
    "workflow_execution_type": "t2i",
    "required_base_volume_size_bytes": 85899345920,
    "runtime_contract": {
      "id": "comfyui-python312-cu121",
      "version": "1.0.0"
    },
    "required_model_assets": [],
    "required_custom_nodes": []
  },
  "resolved_runtime_image": {
    "contract_id": "comfyui-python312-cu121",
    "contract_version": "1.0.0",
    "provisioner_image_ref": "ghcr.io/luma-forge/provisioner-worker@sha256:...",
    "endpoint_image_ref": "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:..."
  }
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

Returns the current worker status with UI-safe diagnostics.

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
  "diagnostic_message": null,
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
  "diagnostic_message": "Downloading model asset SDXL Base 1.0",
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
  "diagnostic_message": "Hugging Face asset download failed",
  "error": {
    "code": "asset_download_failed",
    "reason_code": "asset_download_failed",
    "message": "Hugging Face asset download failed"
  },
  "updated_at": "2026-05-09T00:00:00Z",
  "provisioner_version": "1.0.0"
}
```

## Error Responses

Worker API errors use `code` for broad classification and `reason_code` for specific handling. Safe structured metadata appears under `context` when available. Error payloads must not include bearer tokens, provider API keys, request bodies, raw command output, stack traces, environment dumps, or credential-bearing URLs.

Invalid requests return `400`:

```json
{
  "code": "invalid_request",
  "reason_code": "invalid_request",
  "message": "job_id must be a non-empty string"
}
```

Calling `POST /start` while a job is active returns `409`:

```json
{
  "code": "job_already_running",
  "reason_code": "active_job_exists",
  "message": "Provisioner worker already has an active job.",
  "context": {
    "active_job_id": "workspace-id"
  }
}
```

Unknown endpoints return `404`:

```json
{
  "code": "not_found",
  "reason_code": "endpoint_not_found",
  "message": "Endpoint not found"
}
```

Unauthorized requests return `401`:

```json
{
  "code": "unauthorized",
  "reason_code": "invalid_authorization",
  "message": "Unauthorized."
}
```

Oversized requests return `413`:

```json
{
  "code": "request_too_large",
  "reason_code": "request_body_too_large",
  "message": "Request body is too large.",
  "context": {
    "max_request_bytes": 1048576
  }
}
```
