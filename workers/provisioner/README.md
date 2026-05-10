# LumaForge Provisioner Worker

Container-side worker that prepares a mounted ComfyUI workspace after the native layer calls `POST /start`.

## Local Run

```bash
cd workers/provisioner
PYTHONPATH=src python -m provisioner_worker
```

The worker listens on `127.0.0.1:8000` by default and starts idle. It does not prepare the workspace until `/start` receives a selected Workflow Preset payload.

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
docker build -t luma-forge-provisioner:local .
docker run --rm -p 8000:8000 -v "$PWD/tmp-workspace:/workspace" luma-forge-provisioner:local
```

Optional smoke check:

```bash
cd workers/provisioner
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python -m unittest tests.test_container_smoke
```

## Deployment

See [DEPLOYMENT.md](./DEPLOYMENT.md) for the GitHub Actions worker image deployment workflow, required registry configuration, produced tags, and rollback process.

## API

### `POST /start`

Starts one provisioning job. A second start while a job is active returns `409`.
`workspace_mount_path` must resolve exactly to the worker's configured mount path, set by `LUMA_FORGE_WORKSPACE_MOUNT_PATH` and defaulting to `/workspace`.

```json
{
  "job_id": "workspace-id",
  "workspace_mount_path": "/workspace",
  "workflow_preset": {
    "id": "comfyui-t2i-basic",
    "version": "1.0.0",
    "name": "ComfyUI Text to Image Basic",
    "workflow_execution_type": "t2i",
    "required_base_volume_size_bytes": 85899345920,
    "required_comfyui_source": {
      "source_type": "git",
      "repository_url": "https://github.com/comfyanonymous/ComfyUI.git",
      "revision": "master"
    },
    "required_model_assets": [],
    "required_custom_nodes": []
  }
}
```

### `POST /cancel`

Requests cancellation for the active job.
Cancellation terminates active Git/pip subprocess work and interrupts asset downloads between chunks.

```json
{ "job_id": "workspace-id" }
```

### `GET /status`

Returns the current worker status with UI-safe diagnostics.

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

Terminal states are `succeeded`, `failed`, and `cancelled`. Cancellation in progress reports `cancelling`.

Failure responses include UI-safe error metadata:

```json
{
  "status": "failed",
  "job_id": "workspace-id",
  "phase": null,
  "progress_percent": 56,
  "diagnostic_message": "Hugging Face asset download failed",
  "error": {
    "code": "preparation_failed",
    "message": "Hugging Face asset download failed"
  },
  "updated_at": "2026-05-09T00:00:00Z",
  "provisioner_version": "1.0.0"
}
```

## Error Responses

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
  "code": "job_already_running",
  "message": "Provisioner worker already has an active job.",
  "active_job_id": "workspace-id"
}
```

Unknown endpoints return `404`:

```json
{
  "code": "not_found",
  "message": "Endpoint not found"
}
```
