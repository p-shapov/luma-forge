# RunPod Endpoint Worker

The RunPod Endpoint Worker is the runtime container used behind RunPod Serverless inference endpoints. In this change it preserves the RunPod handler boundary with a temporary minimal `t2i` stub contract:

```json
{
  "execution_type": "t2i",
  "prompt": "a product photo of a small lamp"
}
```

Successful responses are deterministic and UI-safe, and explicitly report that generation is not implemented:

```json
{
  "status": "succeeded",
  "generation": {
    "implemented": false,
    "execution_type": "t2i",
    "message": "Endpoint generation is not implemented in this runtime contract."
  }
}
```

This worker does not require a prepared runtime manifest and does not validate workflow paths, model paths, output directories, image-local ComfyUI paths, or workspace metadata. It does not start ComfyUI, submit workflows, poll execution status, collect outputs, clone repositories, download models, install dependencies, create virtual environments, run pip, mutate image-baked runtime state, or create provider resources at startup or request time.

The endpoint image build may still install runtime contract dependencies under `/opt/luma-forge/runtime` so dependency drift remains visible, but the request handler is currently stubbed and does not execute that runtime.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `LUMA_FORGE_WORKSPACE_MOUNT_PATH` | `/workspace` | Shared prepared workspace volume mount path. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH` | unset | Endpoint-specific workspace mount path override. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS` | `4000` | Maximum accepted prompt length. |
| `LUMA_FORGE_RUNPOD_ENDPOINT_SUPPORTED_EXECUTION_TYPES` | `t2i` | Comma-separated execution types accepted by the stub boundary. |

## Development

```bash
PYTHONPATH=src python3 -m unittest discover -s tests
```

## Container

```bash
docker build -t luma-forge-runpod-endpoint-worker -f Dockerfile ../..
```
