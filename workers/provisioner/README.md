# LumaForge Provisioner Worker

Container-side one-shot worker that prepares a mounted ComfyUI workspace from startup configuration. The worker auto-starts the single provisioning job after configuration validation and exposes `GET /status` for authenticated progress polling.

## Local Run

```bash
cd workers/provisioner
LUMA_FORGE_PROVISIONER_BEARER_TOKEN=local-token-0123456789abcdef0123 \
LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS='[]' \
  PYTHONPATH=src python -m app
```

The worker requires `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` and `LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS` before startup. It listens on `127.0.0.1:8000` and auto-starts the single provisioning job as soon as the HTTP server is created. Clients observe progress with `GET /status`.

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

Optional smoke check:

```bash
cd workers/provisioner
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python -m unittest tests.test_container_smoke
```

Provisioner and endpoint images use separate Dockerfiles. The provisioner Dockerfile does not contain endpoint runtime stages or runtime contract build arguments.

## Deployment

See [Worker Deployment](../DEPLOYMENT.md) for image release triggers, registry conventions, catalog PR ownership, and rollback.

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
| `LUMA_FORGE_PROVISIONER_BEARER_TOKEN` | Required | At least 32 ASCII characters; no whitespace or control characters. |
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
