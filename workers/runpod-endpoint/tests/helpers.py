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
        self.comfyui_root = self.workspace / "ComfyUI"
        self.comfyui_root.mkdir()
        (self.comfyui_root / "main.py").write_text("print('comfy')\n", encoding="utf-8")
        (self.comfyui_root / "models/checkpoints").mkdir(parents=True)
        (self.comfyui_root / "models/checkpoints/sd_xl_base_1.0.safetensors").write_bytes(b"model")
        (self.comfyui_root / "workflows").mkdir()
        (self.comfyui_root / "workflows/t2i.json").write_text(
            json.dumps({"1": {"inputs": {"text": "{{prompt}}"}}}),
            encoding="utf-8",
        )
        self.config = config or EndpointConfig(workspace_mount_path=self.workspace)
        self.comfyui = comfyui or FakeComfyUiClient()
        self.service = GenerationService(config=self.config, comfyui=self.comfyui)

    def close(self):
        self.tempdir.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
