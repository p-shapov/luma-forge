FROM python:3.12-slim

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH=/workspace \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST=127.0.0.1 \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT=8188

WORKDIR /app
COPY pyproject.toml ./
COPY src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
