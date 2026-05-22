FROM python:3.12-slim AS worker-base

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

FROM worker-base AS runtime-builder

ARG LUMA_FORGE_RUNTIME_PYTHON_VERSION=3.12
ARG LUMA_FORGE_COMFYUI_REVISION=8e53f001a492cc818768a308362adbd3d75a1c43
ARG LUMA_FORGE_PYTORCH_INDEX_URL=https://download.pytorch.org/whl/cu121
ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON=["torch==2.5.1","torchvision==0.20.1","torchaudio==2.5.1"]
ARG LUMA_FORGE_BUNDLED_WORKFLOW_PATH=bundled/workflows/comfyui-hidream-o1-dev.json

WORKDIR /runtime-build
COPY ${LUMA_FORGE_BUNDLED_WORKFLOW_PATH} ./workflow.json

RUN apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

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
        --url https://github.com/comfyanonymous/ComfyUI.git \
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
    && printf '{"reports":[]}\n' > /opt/luma-forge/runtime/base-runtime/install-report.json \
    && mkdir -p /opt/luma-forge/runtime/workflows \
    && cp /runtime-build/workflow.json /opt/luma-forge/runtime/workflows/workflow.json \
    && test -s /opt/luma-forge/runtime/workflows/workflow.json

FROM worker-base

WORKDIR /app
COPY --from=runtime-builder /opt/luma-forge/runtime /opt/luma-forge/runtime
COPY workers/runpod-endpoint/pyproject.toml ./
COPY workers/runpod-endpoint/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
