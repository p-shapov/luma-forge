import json
import os
import tempfile
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
from pathlib import Path
import socket
from threading import Event, Thread
from typing import Any

from api.handler import ProvisionerRequestHandler
from app.config import WorkerConfig
from orchestration.preparation_job import JobManager

COMMIT_REVISION = "0123456789abcdef0123456789abcdef01234567"
TEST_BEARER_TOKEN = "test-token-0123456789abcdef012345"
TEST_PROVISIONER_IMAGE_REF = (
    "ghcr.io/luma-forge/provisioner-worker@sha256:"
    "1111111111111111111111111111111111111111111111111111111111111111"
)


def sample_preset() -> dict[str, Any]:
    return {
        "id": "comfyui-t2i-basic",
        "version": "1.0.0",
        "name": "ComfyUI Text to Image Basic",
        "workflow_execution_type": "t2i",
        "required_base_volume_size_bytes": 1,
        "runtime_contract": {
            "id": "comfyui-python312-cu121",
            "version": "1.0.0",
        },
        "required_model_assets": [
            {
                "id": "model",
                "name": "Model",
                "model_asset_kind": "checkpoint",
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


def sample_runtime_image() -> dict[str, Any]:
    return {
        "contract_id": "comfyui-python312-cu121",
        "contract_version": "1.0.0",
        "provisioner_image_ref": TEST_PROVISIONER_IMAGE_REF,
        "endpoint_image_ref": "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222",
    }


def start_payload(*, job_id: str = "job-1", preset: dict[str, Any] | None = None) -> dict[str, Any]:
    return {
        "job_id": job_id,
        "workflow_preset": preset or sample_preset(),
        "resolved_runtime_image": sample_runtime_image(),
    }


class ImmediateProvisioner:
    def prepare(self, request, progress, cancel_event):
        progress("validating_environment", 100, "done")


class RecordingProvisioner:
    def __init__(self, error: Exception | None = None):
        self.error = error
        self.called = False
        self.requests = []

    def prepare(self, request, progress, cancel_event):
        self.called = True
        self.requests.append(request)
        if self.error is not None:
            raise self.error
        progress("validating_environment", 100, "done")


class BlockingProvisioner:
    def __init__(self):
        self.started = Event()
        self.release = Event()

    def prepare(self, request, progress, cancel_event):
        self.started.set()
        progress("materializing_runtime", 10, "blocked")
        while not self.release.is_set() and not cancel_event.is_set():
            self.release.wait(0.01)


def test_config(*, workspace_mount_path: Path | None = None, bearer_token: str = TEST_BEARER_TOKEN, **overrides) -> WorkerConfig:
    image_runtime_root_path = overrides.get("image_runtime_root_path", _image_runtime_fixture())
    config = WorkerConfig.from_env(
        {
            "LUMA_FORGE_PROVISIONER_BEARER_TOKEN": bearer_token,
            "LUMA_FORGE_PROVISIONER_HOST": "127.0.0.1",
            "LUMA_FORGE_PROVISIONER_PORT": "8000",
            "LUMA_FORGE_WORKSPACE_MOUNT_PATH": str(workspace_mount_path or Path("/workspace")),
        }
    )
    return WorkerConfig(
        host=overrides.get("host", config.host),
        port=overrides.get("port", config.port),
        bearer_token=overrides.get("bearer_token", config.bearer_token),
        max_request_bytes=overrides.get("max_request_bytes", config.max_request_bytes),
        git_timeout_seconds=overrides.get("git_timeout_seconds", config.git_timeout_seconds),
        dependency_timeout_seconds=overrides.get(
            "dependency_timeout_seconds",
            config.dependency_timeout_seconds,
        ),
        download_timeout_seconds=overrides.get("download_timeout_seconds", config.download_timeout_seconds),
        workspace_mount_path=overrides.get("workspace_mount_path", config.workspace_mount_path),
        image_runtime_root_path=image_runtime_root_path,
    )


def _image_runtime_fixture() -> Path:
    tempdir = tempfile.TemporaryDirectory()
    root = Path(tempdir.name)
    runtime_root = root / "runtime"
    comfyui = runtime_root / "ComfyUI"
    custom_nodes = comfyui / "custom_nodes"
    venv_bin = runtime_root / ".venv" / "bin"
    custom_nodes.mkdir(parents=True)
    venv_bin.mkdir(parents=True)
    (comfyui / "main.py").write_text("# ComfyUI\n", encoding="utf-8")
    (custom_nodes / "websocket_image_save.py").write_text("# upstream ComfyUI node\n", encoding="utf-8")
    (venv_bin / "python").write_text("#!/usr/bin/env python\n", encoding="utf-8")
    os.chmod(venv_bin / "python", 0o755)
    # Keep the temporary directory alive for the lifetime of the process.
    _IMAGE_RUNTIME_TEMP_DIRS.append(tempdir)
    return runtime_root


_IMAGE_RUNTIME_TEMP_DIRS: list[tempfile.TemporaryDirectory] = []


class ServerFixture:
    def __init__(
        self,
        provisioner: Any,
        *,
        workspace_mount_path: Path | None = None,
        config: WorkerConfig | None = None,
    ):
        config = config or test_config(workspace_mount_path=workspace_mount_path)
        manager = JobManager(provisioner, config=config)

        class Handler(ProvisionerRequestHandler):
            pass

        Handler.manager = manager
        Handler.config = config
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

    def request(
        self,
        method: str,
        path: str,
        body: dict[str, Any] | None = None,
        *,
        headers: dict[str, str] | None = None,
        authorize: bool = True,
    ) -> tuple[int, dict[str, Any]]:
        connection = HTTPConnection("127.0.0.1", self.port, timeout=5)
        payload = None if body is None else json.dumps(body).encode("utf-8")
        request_headers = {"Content-Type": "application/json"} if payload is not None else {}
        if authorize:
            request_headers["Authorization"] = f"Bearer {self.server.RequestHandlerClass.config.bearer_token}"
        request_headers.update(headers or {})
        connection.request(method, path, payload, request_headers)
        response = connection.getresponse()
        data = json.loads(response.read().decode("utf-8"))
        connection.close()
        return response.status, data

    def raw_request(self, request: bytes) -> tuple[int, dict[str, Any]]:
        with socket.create_connection(("127.0.0.1", self.port), timeout=5) as client:
            client.sendall(request)
            response = b""
            while b"\r\n\r\n" not in response:
                response += client.recv(4096)
            headers, body = response.split(b"\r\n\r\n", 1)
            status = int(headers.split(b" ", 2)[1])
            content_length = 0
            for line in headers.split(b"\r\n"):
                if line.lower().startswith(b"content-length:"):
                    content_length = int(line.split(b":", 1)[1].strip())
                    break
            while len(body) < content_length:
                body += client.recv(4096)
            return status, json.loads(body.decode("utf-8"))
