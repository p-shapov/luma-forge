# RunPod Endpoint Worker

The RunPod Endpoint Worker is the runtime container used behind RunPod Serverless inference endpoints. It accepts the current request contract and executes the image-baked ComfyUI workflow through Comfy CLI.

```json
{
  "prompt": "a product photo of a small lamp"
}
```

On the first valid request in a warm worker process, the handler starts ComfyUI lazily with `comfy launch --background`, waits for the local server to become ready, then reuses that server for later jobs. For each job it validates the flat request payload against the image-baked execution schema revision, writes a temporary workflow copy, applies the image-baked execution contract bindings to that copy, runs the patched workflow with `comfy run --json`, fetches local ComfyUI image outputs, writes them under the configured network volume mount, and returns artifact references.

Successful responses are UI-safe:

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

The failed response keeps structured diagnostics in `failure` because the RunPod Python serverless SDK reserves and removes top-level `error` during hosted result normalization. The top-level `error` is still returned as a safe platform failure signal so RunPod marks the job failed; hosted job `output` preserves `status` and `failure`. Stable diagnostic codes include `invalid_request`, `workflow_validation_failed`, `comfyui_launch_failed`, `comfyui_startup_timeout`, `comfyui_workflow_failed`, `comfyui_workflow_timeout`, `comfyui_output_parse_failed`, `comfyui_no_outputs`, `comfyui_output_fetch_failed`, `response_too_large`, and `runtime_failed`. `failure.stage` identifies the failing worker boundary, `failure.retryable` is the worker-owned retry classification, and optional `failure.metadata` contains only bounded primitive values such as an exit status, timeout duration, Comfy CLI JSON failure hints, or normalized and truncated `diagnostic_excerpt` derived from subprocess output. Messages and metadata never include raw stdout, raw stderr, command output, stack traces, credentials, authorization headers, environment dumps, command invocations, or generated image data. Subprocess launch and workflow failures also write full-length captured Comfy CLI output to worker logs for operator debugging after credential-pattern scrubbing; those logs are not part of the hosted response contract.

The worker does not require a provisioner-written runtime manifest. Provisioning remains responsible for prepared workspace directories and model assets only; the endpoint image owns the ComfyUI checkout, Comfy CLI installation, and baked workflow file under `/opt/luma-forge/runtime`.

The runtime image pins ComfyUI to `ea62dc11c9a10dae52186fdcc3da033eb46018a1` and installs PyTorch `2.9.1` CUDA `12.6` wheels (`torch`, `torchvision`, and `torchaudio`) before installing ComfyUI requirements.

The final endpoint image exposes `/opt/luma-forge/runtime/.venv/bin` on `PATH` so Comfy CLI background launches can resolve the image-baked `comfy` executable by name.

## Development

```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

## Container

```bash
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

Optional smoke validation builds or uses an endpoint image and proves the handler imports, ComfyUI checkout, Comfy CLI, and baked workflow are present without running GPU generation:

```bash
LUMA_FORGE_RUN_CONTAINER_SMOKE=1 PYTHONPATH=src python3 -m unittest tests.test_container_smoke
```

## Manual Invocation

After publishing and provisioning a workspace that uses this runtime image, invoke the RunPod serverless endpoint with an input payload shaped like the example above. The response should have `status: "succeeded"`, `generation.implemented: true`, and at least one `runpod_volume` image artifact entry.

## Deployment

RunPod Endpoint Worker images are released through runtime contract deployments. See [Worker Deployment](../DEPLOYMENT.md) for shared release policy, registry conventions, catalog PR ownership, and rollback.

Published runtime images are digest-pinned in Runtime Contracts. Existing deployed Workspaces keep their persisted endpoint image snapshot; endpoint runtime image changes require publishing a new endpoint image and promoting a new Runtime Contracts revision before newly created Workspaces use them.
