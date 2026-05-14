FROM python:3.12-slim AS worker-base

ENV PYTHONUNBUFFERED=1

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

FROM worker-base AS provisioner

ENV LUMA_FORGE_PROVISIONER_HOST=0.0.0.0 \
    LUMA_FORGE_PROVISIONER_PORT=8000 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY workers/provisioner/pyproject.toml ./
COPY workers/provisioner/src ./src

RUN python -m pip install --no-cache-dir .

EXPOSE 8000
CMD ["python", "-m", "provisioner_worker"]

FROM worker-base AS runpod-endpoint

ENV LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH=/workspace \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST=127.0.0.1 \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT=8188

WORKDIR /app
COPY workers/runpod-endpoint/pyproject.toml ./
COPY workers/runpod-endpoint/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
