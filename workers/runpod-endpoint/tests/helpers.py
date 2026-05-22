import base64
import json
from pathlib import Path
import tempfile

from runpod_endpoint_worker.comfyui import ComfyUiClient
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.schemas import ImageOutput
from runpod_endpoint_worker.service import GenerationService


class FakeComfyUiClient(ComfyUiClient):
    def __init__(self, *, available=True, image: ImageOutput | None = None, fail_on_queue: Exception | None = None):
        super().__init__(base_url="http://comfy.test", timeout_seconds=1)
        self.available = available
        self.image = image or ImageOutput(
            mime_type="image/png",
            data=base64.b64encode(b"image").decode("ascii"),
        )
        self.fail_on_queue = fail_on_queue
        self.queued_workflows = []

    def assert_available(self):
        if not self.available:
            from runpod_endpoint_worker.errors import ComfyUiStartupError

            raise ComfyUiStartupError("ComfyUI is unavailable.")

    def queue_prompt(self, workflow):
        if self.fail_on_queue is not None:
            raise self.fail_on_queue
        self.queued_workflows.append(workflow)
        return "prompt-1"

    def wait_for_image(self, prompt_id):
        return self.image


class WorkerFixture:
    def __init__(self, *, config: EndpointConfig | None = None, comfyui: FakeComfyUiClient | None = None):
        self.tempdir = tempfile.TemporaryDirectory()
        self.workspace = Path(self.tempdir.name)
        self.image_runtime_root = self.workspace / "image-runtime"
        self.comfyui_root = self.image_runtime_root / "ComfyUI"
        self.comfyui_root.mkdir(parents=True)
        (self.comfyui_root / "main.py").write_text("print('comfy')\n", encoding="utf-8")
        self.venv_python = self.image_runtime_root / ".venv/bin/python"
        self.venv_python.parent.mkdir(parents=True)
        self.venv_python.write_text("#!/usr/bin/env python\n", encoding="utf-8")
        self.metadata_dir = self.workspace / ".luma-forge"
        self.metadata_dir.mkdir()
        self.base_runtime_dir = self.image_runtime_root / "base-runtime"
        self.base_runtime_dir.mkdir()
        self.pip_freeze_path = self.base_runtime_dir / "pip-freeze.txt"
        self.install_report_path = self.base_runtime_dir / "install-report.json"
        self.pip_freeze_path.write_text("", encoding="utf-8")
        self.install_report_path.write_text('{"reports":[]}\n', encoding="utf-8")
        (self.workspace / "models/checkpoints").mkdir(parents=True)
        (self.workspace / "models/checkpoints/sd_xl_base_1.0.safetensors").write_bytes(b"model")
        (self.workspace / "workflows").mkdir()
        (self.workspace / "workflows/t2i.json").write_text(
            json.dumps({"1": {"inputs": {"text": "{{prompt}}"}}}),
            encoding="utf-8",
        )
        (self.workspace / "output").mkdir()
        self.config = config or EndpointConfig(
            workspace_mount_path=self.workspace,
            image_runtime_root_path=self.image_runtime_root,
        )
        self.write_runtime_manifest()
        self.comfyui = comfyui or FakeComfyUiClient()
        self.service = GenerationService(config=self.config, comfyui=self.comfyui)

    def write_runtime_manifest(self):
        (self.metadata_dir / "runtime-manifest.json").write_text(
            json.dumps(
                {
                    "manifest_kind": "luma_forge_prepared_workspace",
                    "workspace_root": str(self.workspace),
                    "model_asset_paths": [str(self.workspace / "models/checkpoints/sd_xl_base_1.0.safetensors")],
                    "prepared_at": "2026-05-15T00:00:00+00:00",
                }
            ),
            encoding="utf-8",
        )

    def close(self):
        self.tempdir.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
