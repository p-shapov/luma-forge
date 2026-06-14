# LumaForge RunPod Endpoint Worker

RunPod Serverless runtime worker that executes an image-baked ComfyUI workflow through Comfy CLI. The worker accepts the current flat request contract, starts ComfyUI lazily on the first valid job, writes generated images to the configured RunPod network volume, and returns UI-safe artifact references.

## Test

```bash
cd workers/runpod-endpoint
PYTHONPATH=src python3 -m unittest discover -s tests
```

## Lint / Syntax Check

```bash
cd workers/runpod-endpoint
PYTHONPATH=src python3 -m compileall src tests
```

## Container

```bash
cd workers/runpod-endpoint
docker build -t luma-forge-runpod-endpoint-worker -f Dockerfile \
  --build-arg LUMA_FORGE_RUNTIME_PYTHON_VERSION=3.12 \
  --build-arg LUMA_FORGE_COMFYUI_REVISION=ea62dc11c9a10dae52186fdcc3da033eb46018a1 \
  --build-arg LUMA_FORGE_PYTORCH_INDEX_URL=https://download.pytorch.org/whl/cu126 \
  --build-arg 'LUMA_FORGE_PYTORCH_PACKAGES_JSON=["torch==2.9.1","torchvision==0.24.1","torchaudio==2.9.1"]' \
  --build-arg LUMA_FORGE_BUNDLED_WORKFLOW_PATH=bundled/workflows/comfyui-hidream-o1-dev.json \
  --build-arg LUMA_FORGE_WORKFLOW_ID=comfyui-hidream-o1-dev \
  --build-arg LUMA_FORGE_WORKFLOW_VERSION=1.0.0 \
  ../..
```

Optional smoke check:

```bash
cd workers/runpod-endpoint
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python3 -m unittest tests.test_container_smoke
```

The smoke check builds or uses an endpoint image and verifies the handler import, ComfyUI checkout, Comfy CLI, and baked workflow without running GPU generation.

Provisioner and endpoint images use separate Dockerfiles. The endpoint Dockerfile owns the ComfyUI checkout, Comfy CLI installation, baked workflow, and baked execution contract under `/opt/luma-forge/runtime`.

## Deployment

Publish an endpoint image by pushing a `runpod-endpoint-v*` tag or running the `Deploy RunPod Endpoint` GitHub Actions workflow manually with `workflow_id` and `workflow_version`.

The workflow resolves the selected workflow revision's runtime preset, validates the endpoint contract tooling and endpoint package, builds `ghcr.io/<owner>/<repo>/runpod-endpoint-worker`, and resolves the pushed digest. The endpoint contract id matches the Workflow Preset id, so the build bakes `bundled/workflows/{contract.id}.json` into `/opt/luma-forge/runtime/workflows/workflow.json`.

After publication, the workflow opens a Runtime Contracts promotion PR. That PR appends the new endpoint contract revision to `bundled/runtime-contracts.json` and updates the matching Workflow Preset in `bundled/workflow-catalog.json`.

New Workspaces use the image only after the promotion PR is reviewed, merged, and bundled into the app. Existing Workspaces remain pinned to their persisted endpoint image snapshot.

## Runtime Configuration

Runtime configuration is baked into the endpoint image instead of loaded from startup environment. The worker does not require a provisioner-written runtime manifest.

| Value | Source |
| --- | --- |
| ComfyUI checkout | `/opt/luma-forge/runtime/ComfyUI` |
| Comfy CLI | `/opt/luma-forge/runtime/.venv/bin/comfy` |
| Workflow file | `/opt/luma-forge/runtime/workflows/workflow.json` |
| Execution contract | `/opt/luma-forge/runtime/contracts/execution-contract.json` |
| Workspace mount | `/workspace` |
| Artifact output prefix | `luma-forge/outputs/jobs/<job-id>/` |

The runtime image pins ComfyUI to `ea62dc11c9a10dae52186fdcc3da033eb46018a1` and installs PyTorch `2.9.1` CUDA `12.6` wheels before installing ComfyUI requirements. The final image exposes `/opt/luma-forge/runtime/.venv/bin` on `PATH` so `comfy` resolves by name.

## API

The worker is invoked by RunPod Serverless. The job `input` must match the baked execution schema for the selected workflow.

Successful responses are UI-safe and include generated image artifacts:

```json
{
  "status": "succeeded",
  "generation": {
    "implemented": true,
    "images": [
      {
        "filename": "ComfyUI_00001_.png",
        "mime_type": "image/png",
        "byte_size": 1234567,
        "sha256": "...",
        "artifact_uri": "runpod-volume://luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png",
        "storage": {
          "type": "runpod_volume",
          "relative_path": "luma-forge/outputs/jobs/job-123/0001/ComfyUI_00001_.png"
        }
      }
    ]
  }
}
```

## Error Responses

Failed responses include a UI-safe `failure` object and a top-level `error` string so RunPod marks the job failed. `failure.code` is the stable specific classifier, `failure.stage` identifies the failing worker boundary, and `failure.retryable` is the worker-owned retry classification:

```json
{
  "status": "failed",
  "failure": {
    "code": "comfyui_workflow_failed",
    "message": "ComfyUI workflow execution failed. Process exited with status 1.",
    "stage": "workflow_execution",
    "retryable": false,
    "metadata": {
      "exit_status": 1
    }
  },
  "error": "comfyui_workflow_failed: ComfyUI workflow execution failed. Process exited with status 1."
}
```

Endpoint failure codes:

| Code | Stage | Retryable | Meaning |
| --- | --- | --- | --- |
| `invalid_request` | `request_validation` | No | The RunPod job `input` is not an object, contains unknown inputs, misses required inputs, or contains invalid input values. |
| `workflow_validation_failed` | `workflow_validation` | No | The baked workflow, execution contract, or execution schema is missing, unreadable, malformed, or internally inconsistent. |
| `comfyui_startup_failed` | `comfyui_startup` | Yes | Generic ComfyUI startup failure fallback. |
| `comfyui_launch_failed` | `comfyui_launch` | No | `comfy launch --background` failed before ComfyUI became ready. |
| `comfyui_startup_timeout` | `comfyui_startup` | Yes | ComfyUI did not become ready before the startup timeout. |
| `comfyui_execution_failed` | `workflow_execution` | No | Generic ComfyUI execution failure fallback. |
| `comfyui_workflow_failed` | `workflow_execution` | No | `comfy run --json` failed or did not report workflow completion. |
| `comfyui_workflow_timeout` | `workflow_execution` | Yes | Workflow execution exceeded the worker timeout. |
| `comfyui_output_parse_failed` | `output_parse` | No | Comfy CLI emitted malformed or unexpected JSON events. |
| `comfyui_no_outputs` | `output_parse` | No | ComfyUI completed without image outputs. |
| `comfyui_output_fetch_failed` | `output_fetch` | Yes | Generated output could not be fetched from ComfyUI, persisted to the workspace volume, or cleaned up safely. |
| `response_too_large` | `response_size` | No | Generated artifacts or response metadata exceed worker size limits. |
| `runtime_failed` | `runtime` | Yes | An unexpected endpoint worker exception was caught and converted to a safe failure response. |
| `runpod_endpoint_worker_error` | `runtime` | No | Generic endpoint worker error fallback. |

Safe structured metadata may appear under `failure.metadata` for `exit_status`, `timeout_seconds`, `diagnostic_excerpt`, normalized ComfyUI error fields, and missing model paths.

Error payloads must not include raw stdout, raw stderr, command output, stack traces, credentials, authorization headers, environment dumps, command invocations, credential-bearing URLs, or generated image data.
