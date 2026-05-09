import json
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
from pathlib import Path
from threading import Event, Thread
from typing import Any

from provisioner_worker.api import ProvisionerRequestHandler
from provisioner_worker.job_manager import JobManager
from provisioner_worker.preparer import Provisioner


def sample_preset(*, model_size: int = 0) -> dict[str, Any]:
    return {
        "id": "comfyui-t2i-basic",
        "version": "1.0.0",
        "name": "ComfyUI Text to Image Basic",
        "workflow_execution_type": "t2i",
        "required_base_volume_size_bytes": 1,
        "required_comfyui_source": {
            "source_type": "git",
            "repository_url": "https://example.test/ComfyUI.git",
            "revision": "main",
        },
        "required_model_assets": [
            {
                "id": "model",
                "name": "Model",
                "model_asset_kind": "checkpoint",
                "file_size_bytes": model_size,
                "download_source": {
                    "source_type": "huggingface",
                    "repository_id": "owner/model",
                    "file_path": "model.safetensors",
                    "revision": "main",
                },
                "install": {
                    "comfyui_relative_path": "models/checkpoints/model.safetensors",
                },
            },
        ],
        "required_custom_nodes": [],
    }


def start_payload(tmp_path: Path, *, job_id: str = "job-1", preset: dict[str, Any] | None = None) -> dict[str, Any]:
    return {
        "job_id": job_id,
        "workspace_mount_path": str(tmp_path),
        "workflow_preset": preset or sample_preset(),
    }


class ImmediateProvisioner(Provisioner):
    def prepare(self, request, progress, cancel_event):
        progress("validating_environment", 100, "done")


class BlockingProvisioner(Provisioner):
    def __init__(self):
        self.started = Event()
        self.release = Event()

    def prepare(self, request, progress, cancel_event):
        self.started.set()
        progress("installing_comfyui", 10, "blocked")
        while not self.release.is_set() and not cancel_event.is_set():
            self.release.wait(0.01)


class ServerFixture:
    def __init__(self, provisioner: Provisioner, *, workspace_mount_path: Path | None = None):
        manager = JobManager(provisioner, workspace_mount_path=workspace_mount_path)

        class Handler(ProvisionerRequestHandler):
            pass

        Handler.manager = manager
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = Thread(target=self.server.serve_forever, daemon=True)
        self.port = self.server.server_port

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, exc_type, exc, tb):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    def request(self, method: str, path: str, body: dict[str, Any] | None = None) -> tuple[int, dict[str, Any]]:
        connection = HTTPConnection("127.0.0.1", self.port, timeout=5)
        payload = None if body is None else json.dumps(body).encode("utf-8")
        headers = {"Content-Type": "application/json"} if payload is not None else {}
        connection.request(method, path, payload, headers)
        response = connection.getresponse()
        data = json.loads(response.read().decode("utf-8"))
        connection.close()
        return response.status, data
