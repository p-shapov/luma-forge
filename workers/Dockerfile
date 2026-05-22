FROM python:3.12-slim AS worker-base

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

FROM worker-base AS runtime-builder

ARG LUMA_FORGE_RUNTIME_PYTHON_VERSION=3.12
ARG LUMA_FORGE_RUNTIME_PLATFORM=linux-x86_64-cuda
ARG LUMA_FORGE_COMFYUI_REPOSITORY=https://github.com/comfyanonymous/ComfyUI.git
ARG LUMA_FORGE_COMFYUI_REVISION=aa9d2fc713664e9ffe37763f4c9240c0c3eda667
ARG LUMA_FORGE_PYTORCH_INDEX_URL=https://download.pytorch.org/whl/cu121
ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON=["torch==2.5.1","torchvision==0.20.1","torchaudio==2.5.1"]
ARG LUMA_FORGE_BASE_REQUIREMENTS_JSON=["requirements.txt"]

WORKDIR /runtime-build

RUN mkdir -p /opt/luma-forge/runtime \
    && python -m venv --copies /opt/luma-forge/runtime/.venv \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip install --no-cache-dir --upgrade pip \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip install --no-cache-dir comfy-cli==1.7.3

RUN test "$LUMA_FORGE_RUNTIME_PYTHON_VERSION" = "3.12" \
    && /opt/luma-forge/runtime/.venv/bin/python -c 'import json, os, subprocess; packages = json.loads(os.environ["LUMA_FORGE_PYTORCH_PACKAGES_JSON"]); subprocess.check_call(["/opt/luma-forge/runtime/.venv/bin/python", "-m", "pip", "install", "--no-cache-dir", "--index-url", os.environ["LUMA_FORGE_PYTORCH_INDEX_URL"], *packages])'

RUN export VIRTUAL_ENV=/opt/luma-forge/runtime/.venv \
    && export PATH="${VIRTUAL_ENV}/bin:${PATH}" \
    && comfy --skip-prompt tracking disable \
    && comfy --skip-prompt --workspace /opt/luma-forge/runtime/ComfyUI install \
        --url "$LUMA_FORGE_COMFYUI_REPOSITORY" \
        --version nightly \
        --commit "$LUMA_FORGE_COMFYUI_REVISION" \
        --nvidia \
        --skip-manager \
        --skip-torch-or-directml \
    && test -f /opt/luma-forge/runtime/ComfyUI/main.py \
    && test -x /opt/luma-forge/runtime/.venv/bin/python \
    && test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager

RUN mkdir -p /opt/luma-forge/runtime/base-runtime \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip freeze > /opt/luma-forge/runtime/base-runtime/pip-freeze.txt \
    && printf '{"reports":[]}\n' > /opt/luma-forge/runtime/base-runtime/install-report.json

FROM worker-base AS provisioner

ENV LUMA_FORGE_PROVISIONER_HOST=0.0.0.0 \
    LUMA_FORGE_PROVISIONER_PORT=8000

RUN apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=runtime-builder /opt/luma-forge/runtime /opt/luma-forge/runtime
COPY workers/provisioner/pyproject.toml ./
COPY workers/provisioner/src ./src

RUN python -m pip install --no-cache-dir .

EXPOSE 8000
CMD ["python", "-m", "app"]

FROM worker-base AS runpod-endpoint

ENV LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST=127.0.0.1 \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT=8188

WORKDIR /app
COPY --from=runtime-builder /opt/luma-forge/runtime /opt/luma-forge/runtime
COPY workers/runpod-endpoint/pyproject.toml ./
COPY workers/runpod-endpoint/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
