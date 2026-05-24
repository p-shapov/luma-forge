# RunPod Endpoint Worker

The RunPod Endpoint Worker is the runtime container used behind RunPod Serverless inference endpoints. It accepts the current temporary `t2i` request contract and executes the image-baked HiDream O1 Dev ComfyUI workflow through Comfy CLI.

```json
{
  "execution_type": "t2i",
  "prompt": "a product photo of a small lamp"
}
```

On the first valid request in a warm worker process, the handler starts ComfyUI lazily with `comfy launch --background`, waits for the local server to become ready, then reuses that server for later jobs. For each job it writes a temporary workflow copy, patches HiDream node `171` (`User Prompt`) with the request prompt, sets node `154` (`Switch to Image Edit`) to `false`, sets node `177` (`Enable Prompt Refine?`) to `false`, runs the patched workflow with `comfy run --json`, fetches local ComfyUI image outputs, writes them under the configured network volume mount, and returns artifact references.

Successful responses are UI-safe:

```json
{
  "status": "succeeded",
  "generation": {
    "implemented": true,
    "execution_type": "t2i",
    "images": [
      {
        "filename": "ComfyUI_00001_.png",
        "mime_type": "image/png",
        "byte_size": 1234567,
        "sha256": "...",
        "artifact_uri": "runpod-volume://luma-forge/outputs/jobs/job-123/ComfyUI_00001_.png",
        "storage": {
          "type": "runpod_volume",
          "relative_path": "luma-forge/outputs/jobs/job-123/ComfyUI_00001_.png"
        }
      }
    ]
  }
}
```

Failed responses include UI-safe diagnostic metadata:

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

The failed response keeps structured diagnostics in `failure` because the RunPod Python serverless SDK reserves and removes top-level `error` during hosted result normalization. The top-level `error` is still returned as a safe platform failure signal so RunPod marks the job failed; hosted job `output` preserves `status` and `failure`. Stable diagnostic codes include `invalid_request`, `unsupported_execution_type`, `workflow_validation_failed`, `comfyui_launch_failed`, `comfyui_startup_timeout`, `comfyui_workflow_failed`, `comfyui_workflow_timeout`, `comfyui_output_parse_failed`, `comfyui_no_outputs`, `comfyui_output_fetch_failed`, `response_too_large`, and `runtime_failed`. `failure.stage` identifies the failing worker boundary, `failure.retryable` is the worker-owned retry classification, and optional `failure.metadata` contains only bounded primitive values such as an exit status, timeout duration, Comfy CLI JSON failure hints, or normalized and truncated `diagnostic_excerpt` derived from subprocess output. Messages and metadata never include raw stdout, raw stderr, command output, stack traces, credentials, authorization headers, environment dumps, command invocations, or generated image data. Subprocess launch and workflow failures also write full-length captured Comfy CLI output to worker logs for operator debugging after credential-pattern scrubbing; those logs are not part of the hosted response contract.

The worker does not require a provisioner-written runtime manifest. Provisioning remains responsible for prepared workspace directories and model assets only; the endpoint image owns the ComfyUI checkout, Comfy CLI installation, and baked workflow file under `/opt/luma-forge/runtime`.

The runtime image pins ComfyUI to `ea62dc11c9a10dae52186fdcc3da033eb46018a1` and installs PyTorch `2.9.1` CUDA `12.6` wheels (`torch`, `torchvision`, and `torchaudio`) before installing ComfyUI requirements.

The final endpoint image exposes `/opt/luma-forge/runtime/.venv/bin` on `PATH` so Comfy CLI background launches can resolve the image-baked `comfy` executable by name.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `LUMA_FORGE_WORKSPACE_MOUNT_PATH` | `/workspace` | Shared prepared workspace volume mount path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH` | unset | Endpoint-specific workspace mount path override. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFY_CLI_PATH` | `/opt/luma-forge/runtime/.venv/bin/comfy` | Comfy CLI executable path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PATH` | `/opt/luma-forge/runtime/ComfyUI` | Image-local ComfyUI checkout path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_WORKFLOW_PATH` | `/opt/luma-forge/runtime/workflows/workflow.json` | Image-local baked UI workflow path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST` | `127.0.0.1` | Local ComfyUI host. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT` | `8188` | Local ComfyUI port. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS` | `300` | Time allowed for lazy ComfyUI startup readiness. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_TIMEOUT_SECONDS` | `900` | Time allowed for `comfy run --json`. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_MAX_RESPONSE_BYTES` | `9000000` | Maximum allowed JSON response metadata size. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS` | `4000` | Maximum accepted prompt length. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_SUPPORTED_EXECUTION_TYPES` | `t2i` | Comma-separated execution types accepted by the endpoint boundary. |

## Development

```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

## Container

```bash
docker build -t luma-forge-runpod-endpoint-worker -f Dockerfile ../..
```

Optional smoke validation builds or uses an endpoint image and proves the handler imports, ComfyUI checkout, Comfy CLI, and baked workflow are present without running GPU generation:

```bash
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python3 -m unittest tests.test_container_smoke
```

## Manual Invocation

After publishing and provisioning a workspace that uses this runtime image, invoke the RunPod serverless endpoint with an input payload shaped like the `t2i` example above. The response should have `status: "succeeded"`, `generation.implemented: true`, and at least one `runpod_volume` image artifact entry.

## Deployment

RunPod Endpoint Worker images are released through runtime contract deployments. See [Worker Deployment](../DEPLOYMENT.md) for shared release policy, registry conventions, catalog PR ownership, and rollback.

Published runtime images are digest-pinned in the Runtime Catalog. Existing deployed Workspaces keep their persisted endpoint image snapshot; endpoint runtime image changes require publishing a new endpoint image and promoting a new Runtime Catalog revision before newly created Workspaces use them.
