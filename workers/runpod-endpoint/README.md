# RunPod Endpoint Worker

The RunPod Endpoint Worker is the runtime container used behind RunPod Serverless inference endpoints. It supports the temporary minimal `t2i` generation contract:

```json
{
  "execution_type": "t2i",
  "prompt": "a product photo of a small lamp"
}
```

Successful responses return exactly one image as MIME type plus base64 data:

```json
{
  "status": "succeeded",
  "image": {
    "mime_type": "image/png",
    "data": "..."
  }
}
```

This worker assumes the workspace volume was already prepared by the Provisioner Worker. It does not clone ComfyUI, download models, install dependencies, create virtual environments, run pip, or create provider resources. It starts the prepared local ComfyUI process lazily before the first valid generation request, waits for `/system_stats`, and reuses the process for later jobs in the same warm worker.

The prepared workspace must include the volume-local runtime environment:

```text
/workspace/
  ComfyUI/
  .venv/
  .luma-forge/
    runtime.json
    pip-freeze.txt
    install-report.json
```

The endpoint validates `/workspace/.luma-forge/runtime.json` and starts ComfyUI through the manifest-declared volume-local interpreter, normally `/workspace/.venv/bin/python`.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH` | `/workspace` | Prepared workspace volume mount path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST` | `127.0.0.1` | ComfyUI host inside the endpoint container. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT` | `8188` | ComfyUI HTTP port inside the endpoint container. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS` | `120` | Maximum wait for the local ComfyUI process to become HTTP-ready. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS` | `4000` | Maximum accepted prompt length. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_GENERATION_TIMEOUT_SECONDS` | `300` | Maximum ComfyUI generation wait time. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_WORKFLOW_RELATIVE_PATH` | `workflows/t2i.json` | ComfyUI workflow JSON path relative to the ComfyUI root. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_REQUIRED_MODEL_PATHS` | `models/checkpoints/sd_xl_base_1.0.safetensors` | Comma-separated ComfyUI-relative model paths required before generation. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_REQUIRED_CUSTOM_NODE_PATHS` | empty | Comma-separated ComfyUI-relative custom node paths required before generation. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_T2I_PROMPT_NODE_ID` | empty | Optional ComfyUI node id to receive the prompt. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_T2I_PROMPT_INPUT_KEY` | `text` | Input key used when prompt node id is configured. |

## Development

```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

## Container

```bash
docker build -t luma-forge-runpod-endpoint-worker -f ../Dockerfile --target runpod-endpoint ../..
```
