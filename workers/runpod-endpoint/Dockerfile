FROM python:3.12-slim AS worker-base

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

FROM worker-base AS runtime-builder

ARG LUMA_FORGE_RUNTIME_PYTHON_VERSION
ARG LUMA_FORGE_COMFYUI_REVISION
ARG LUMA_FORGE_PYTORCH_INDEX_URL
ARG LUMA_FORGE_PYTORCH_PACKAGES_JSON
ARG LUMA_FORGE_BUNDLED_WORKFLOW_PATH
ARG LUMA_FORGE_WORKFLOW_ID
ARG LUMA_FORGE_WORKFLOW_VERSION

WORKDIR /runtime-build
COPY ${LUMA_FORGE_BUNDLED_WORKFLOW_PATH} ./workflow.json
COPY bundled/workflow-catalog.json ./workflow-catalog.json
COPY bundled/execution-schemas.json ./execution-schemas.json
COPY workers/runpod-endpoint/src/runpod_endpoint_worker/build_metadata.py ./extract_runtime_metadata.py

RUN test -n "$LUMA_FORGE_RUNTIME_PYTHON_VERSION" \
    && test -n "$LUMA_FORGE_COMFYUI_REVISION" \
    && test -n "$LUMA_FORGE_PYTORCH_INDEX_URL" \
    && test -n "$LUMA_FORGE_PYTORCH_PACKAGES_JSON" \
    && test -n "$LUMA_FORGE_BUNDLED_WORKFLOW_PATH" \
    && test -n "$LUMA_FORGE_WORKFLOW_ID" \
    && test -n "$LUMA_FORGE_WORKFLOW_VERSION"

RUN python /runtime-build/extract_runtime_metadata.py \
    --workflow-catalog /runtime-build/workflow-catalog.json \
    --execution-schemas /runtime-build/execution-schemas.json \
    --workflow-id "$LUMA_FORGE_WORKFLOW_ID" \
    --workflow-version "$LUMA_FORGE_WORKFLOW_VERSION" \
    --execution-contract-output /runtime-build/execution-contract.json

RUN mkdir -p /opt/luma-forge/runtime \
    && python -m venv --copies /opt/luma-forge/runtime/.venv \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip install --no-cache-dir --upgrade pip \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip install --no-cache-dir comfy-cli==1.10.3 \
    && /opt/luma-forge/runtime/.venv/bin/comfy --version

RUN test "$LUMA_FORGE_RUNTIME_PYTHON_VERSION" = "3.12" \
    && /opt/luma-forge/runtime/.venv/bin/python -c 'import json, os, subprocess; packages = json.loads(os.environ["LUMA_FORGE_PYTORCH_PACKAGES_JSON"]); subprocess.check_call(["/opt/luma-forge/runtime/.venv/bin/python", "-m", "pip", "install", "--no-cache-dir", "--index-url", os.environ["LUMA_FORGE_PYTORCH_INDEX_URL"], *packages])'

RUN export VIRTUAL_ENV=/opt/luma-forge/runtime/.venv \
    && export PATH="${VIRTUAL_ENV}/bin:${PATH}" \
    && comfy --skip-prompt tracking disable \
    && comfy --skip-prompt --workspace /opt/luma-forge/runtime/ComfyUI install \
        --url https://github.com/Comfy-Org/ComfyUI.git \
        --version nightly \
        --commit "$LUMA_FORGE_COMFYUI_REVISION" \
        --nvidia \
        --skip-manager \
        --skip-torch-or-directml \
    && test -f /opt/luma-forge/runtime/ComfyUI/main.py \
    && test -x /opt/luma-forge/runtime/.venv/bin/python \
    && test -x /opt/luma-forge/runtime/.venv/bin/comfy \
    && test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager

RUN mkdir -p /opt/luma-forge/runtime/base-runtime \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip freeze > /opt/luma-forge/runtime/base-runtime/pip-freeze.txt \
    && printf '{"reports":[]}\n' > /opt/luma-forge/runtime/base-runtime/install-report.json \
    && mkdir -p /opt/luma-forge/runtime/workflows \
    && cp /runtime-build/workflow.json /opt/luma-forge/runtime/workflows/workflow.json \
    && test -s /opt/luma-forge/runtime/workflows/workflow.json \
    && mkdir -p /opt/luma-forge/runtime/contracts \
    && cp /runtime-build/execution-contract.json /opt/luma-forge/runtime/contracts/execution-contract.json \
    && test -s /opt/luma-forge/runtime/contracts/execution-contract.json

FROM worker-base

ENV VIRTUAL_ENV=/opt/luma-forge/runtime/.venv
ENV PATH="${VIRTUAL_ENV}/bin:${PATH}"

WORKDIR /app
COPY --from=runtime-builder /opt/luma-forge/runtime /opt/luma-forge/runtime
COPY workers/runpod-endpoint/pyproject.toml ./
COPY workers/runpod-endpoint/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
