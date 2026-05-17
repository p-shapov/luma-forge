FROM python:3.12-slim AS worker-base

ENV PYTHONUNBUFFERED=1 \
    LUMA_FORGE_WORKSPACE_MOUNT_PATH=/workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

FROM worker-base AS runtime-builder

ARG LUMA_FORGE_RUNTIME_CONTRACT_ID=comfyui-python312-cu121
ARG LUMA_FORGE_RUNTIME_CONTRACT_VERSION=1.0.0
ARG LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION=2026.05.16-001
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
    && /opt/luma-forge/runtime/.venv/bin/python -m pip install --no-cache-dir --upgrade pip

RUN test "$LUMA_FORGE_RUNTIME_PYTHON_VERSION" = "3.12" \
    && /opt/luma-forge/runtime/.venv/bin/python -c 'import json, os, subprocess; packages = json.loads(os.environ["LUMA_FORGE_PYTORCH_PACKAGES_JSON"]); subprocess.check_call(["/opt/luma-forge/runtime/.venv/bin/python", "-m", "pip", "install", "--no-cache-dir", "--index-url", os.environ["LUMA_FORGE_PYTORCH_INDEX_URL"], *packages])'

RUN git clone "$LUMA_FORGE_COMFYUI_REPOSITORY" /opt/luma-forge/runtime/ComfyUI \
    && cd /opt/luma-forge/runtime/ComfyUI \
    && git checkout "$LUMA_FORGE_COMFYUI_REVISION"

RUN /opt/luma-forge/runtime/.venv/bin/python -c 'import json, os, pathlib, subprocess; comfyui = pathlib.Path("/opt/luma-forge/runtime/ComfyUI").resolve(); requirements = json.loads(os.environ["LUMA_FORGE_BASE_REQUIREMENTS_JSON"]); [(target.is_relative_to(comfyui) or (_ for _ in ()).throw(SystemExit(f"unsafe base requirement path: {requirement}"))) and subprocess.check_call(["/opt/luma-forge/runtime/.venv/bin/python", "-m", "pip", "install", "--no-cache-dir", "-r", str(target)]) for requirement in requirements for target in [(comfyui / requirement).resolve()]]'

RUN mkdir -p /opt/luma-forge/runtime/base-runtime \
    && /opt/luma-forge/runtime/.venv/bin/python -m pip freeze > /opt/luma-forge/runtime/base-runtime/pip-freeze.txt \
    && printf '{"reports":[]}\n' > /opt/luma-forge/runtime/base-runtime/install-report.json \
    && /opt/luma-forge/runtime/.venv/bin/python -c 'import json, os; metadata = {"contract_id": os.environ["LUMA_FORGE_RUNTIME_CONTRACT_ID"], "contract_version": os.environ["LUMA_FORGE_RUNTIME_CONTRACT_VERSION"], "implementation_revision": os.environ["LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION"], "environment_kind": "image_baked_comfyui_runtime", "python_version": os.environ["LUMA_FORGE_RUNTIME_PYTHON_VERSION"], "platform": os.environ["LUMA_FORGE_RUNTIME_PLATFORM"], "comfyui_revision": os.environ["LUMA_FORGE_COMFYUI_REVISION"], "image_runtime_root_path": "/opt/luma-forge/runtime", "image_python_interpreter_path": "/opt/luma-forge/runtime/.venv/bin/python", "image_comfyui_root_path": "/opt/luma-forge/runtime/ComfyUI", "image_base_dependency_record_paths": ["base-runtime/pip-freeze.txt", "base-runtime/install-report.json"], "runtime_manifest_compatibility": {"manifest_version": "1"}, "workspace_overlay_policy": {"python_overlay_path": ".luma-forge/python-overlay", "import_path_precedence": "overlay_first", "protected_package_names": ["torch", "torchvision", "torchaudio"], "protected_package_prefixes": ["nvidia-"]}, "pytorch_index_url": os.environ["LUMA_FORGE_PYTORCH_INDEX_URL"], "pytorch_packages": json.loads(os.environ["LUMA_FORGE_PYTORCH_PACKAGES_JSON"]), "base_requirements": json.loads(os.environ["LUMA_FORGE_BASE_REQUIREMENTS_JSON"])}; payload = json.dumps(metadata, sort_keys=True) + "\n"; open("/opt/luma-forge/runtime/runtime-metadata.json", "w", encoding="utf-8").write(payload); open("/opt/luma-forge/runtime/runtime-contract.json", "w", encoding="utf-8").write(payload)'

FROM worker-base AS provisioner

ARG LUMA_FORGE_RUNTIME_CONTRACT_ID=comfyui-python312-cu121
ARG LUMA_FORGE_RUNTIME_CONTRACT_VERSION=1.0.0
ARG LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION=2026.05.16-001

ENV LUMA_FORGE_PROVISIONER_HOST=0.0.0.0 \
    LUMA_FORGE_PROVISIONER_PORT=8000 \
    LUMA_FORGE_IMAGE_RUNTIME_ROOT=/opt/luma-forge/runtime \
    LUMA_FORGE_RUNTIME_CONTRACT_ID=$LUMA_FORGE_RUNTIME_CONTRACT_ID \
    LUMA_FORGE_RUNTIME_CONTRACT_VERSION=$LUMA_FORGE_RUNTIME_CONTRACT_VERSION \
    LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION=$LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION

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

ARG LUMA_FORGE_RUNTIME_CONTRACT_ID=comfyui-python312-cu121
ARG LUMA_FORGE_RUNTIME_CONTRACT_VERSION=1.0.0
ARG LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION=2026.05.16-001

ENV LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST=127.0.0.1 \
    LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT=8188 \
    LUMA_FORGE_IMAGE_RUNTIME_ROOT=/opt/luma-forge/runtime \
    LUMA_FORGE_RUNTIME_CONTRACT_ID=$LUMA_FORGE_RUNTIME_CONTRACT_ID \
    LUMA_FORGE_RUNTIME_CONTRACT_VERSION=$LUMA_FORGE_RUNTIME_CONTRACT_VERSION \
    LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION=$LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION

WORKDIR /app
COPY --from=runtime-builder /opt/luma-forge/runtime /opt/luma-forge/runtime
COPY workers/runpod-endpoint/pyproject.toml ./
COPY workers/runpod-endpoint/src ./src

RUN python -m pip install --no-cache-dir .

CMD ["python", "-m", "runpod_endpoint_worker"]
