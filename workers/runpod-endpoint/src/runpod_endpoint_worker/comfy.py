from __future__ import annotations

import base64
import json
import mimetypes
import subprocess
import tempfile
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.parse import urlencode
from urllib.request import Request, urlopen

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import ComfyExecutionError, ComfyStartupError
from runpod_endpoint_worker.schemas import GenerationImage, GenerationRequest
from runpod_endpoint_worker.workflow import write_patched_workflow


_COMFY_STARTUP_LOCK = threading.Lock()


@dataclass(frozen=True)
class ComfyImageOutput:
    filename: str
    subfolder: str
    output_type: str


class HttpClient:
    def get_json(self, url: str, timeout: float) -> Any:
        request = Request(url, method="GET")
        with urlopen(request, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))

    def get_bytes(self, url: str, timeout: float) -> bytes:
        request = Request(url, method="GET")
        with urlopen(request, timeout=timeout) as response:
            return response.read()


@dataclass
class ComfyRuntime:
    config: EndpointConfig
    http_client: HttpClient
    ready: bool = False

    def ensure_ready(self) -> None:
        if self._is_ready():
            self.ready = True
            return

        with _COMFY_STARTUP_LOCK:
            if self._is_ready():
                self.ready = True
                return

            self._launch()
            deadline = time.monotonic() + self.config.comfyui_startup_timeout_seconds
            while time.monotonic() <= deadline:
                if self._is_ready():
                    self.ready = True
                    return
                time.sleep(self.config.comfy_ui_ready_poll_seconds)

            raise ComfyStartupError("ComfyUI did not become ready before the startup timeout.")

    def _launch(self) -> None:
        extra_model_paths_config = _write_extra_model_paths_config(self.config)
        command = [
            str(self.config.comfy_cli_path),
            "--skip-prompt",
            "--workspace",
            str(self.config.comfyui_path),
            "launch",
            "--background",
            "--",
            "--listen",
            self.config.comfyui_host,
            "--port",
            str(self.config.comfyui_port),
            "--extra-model-paths-config",
            str(extra_model_paths_config),
        ]
        try:
            subprocess.run(
                command,
                check=True,
                capture_output=True,
                text=True,
                timeout=self.config.comfyui_startup_timeout_seconds,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise ComfyStartupError("ComfyUI failed to start.") from error

    def _is_ready(self) -> bool:
        try:
            self.http_client.get_json(_url(self.config, "/system_stats"), timeout=2)
        except Exception:
            return False
        return True


@dataclass
class ComfyExecutor:
    config: EndpointConfig
    runtime: ComfyRuntime
    http_client: HttpClient

    @classmethod
    def from_config(cls, config: EndpointConfig) -> "ComfyExecutor":
        http_client = HttpClient()
        return cls(
            config=config,
            runtime=ComfyRuntime(config=config, http_client=http_client),
            http_client=http_client,
        )

    def generate(self, request: GenerationRequest) -> list[GenerationImage]:
        self.runtime.ensure_ready()
        with tempfile.TemporaryDirectory(prefix="luma-forge-workflow-") as directory:
            patched_workflow = Path(directory) / "workflow.json"
            write_patched_workflow(self.config.workflow_path, patched_workflow, request.prompt)
            outputs = self._run_workflow(patched_workflow)

        if not outputs:
            raise ComfyExecutionError("ComfyUI completed without image outputs.")

        images = [self._fetch_image(output) for output in outputs]
        if sum(len(image.data_base64) for image in images) > self.config.max_response_bytes:
            raise ComfyExecutionError("Generated image response exceeds the inline response size limit.")
        return images

    def _run_workflow(self, workflow_path: Path) -> list[ComfyImageOutput]:
        command = [
            str(self.config.comfy_cli_path),
            "--skip-prompt",
            "--workspace",
            str(self.config.comfyui_path),
            "run",
            "--workflow",
            str(workflow_path),
            "--host",
            _connect_host(self.config.comfyui_host),
            "--port",
            str(self.config.comfyui_port),
            "--wait",
            "--timeout",
            str(self.config.execution_timeout_seconds),
            "--json",
        ]
        try:
            completed = subprocess.run(
                command,
                check=True,
                capture_output=True,
                text=True,
                timeout=self.config.execution_timeout_seconds,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise ComfyExecutionError("ComfyUI workflow execution failed.") from error
        return parse_comfy_run_events(completed.stdout)

    def _fetch_image(self, output: ComfyImageOutput) -> GenerationImage:
        query = urlencode(
            {
                "filename": output.filename,
                "subfolder": output.subfolder,
                "type": output.output_type,
            }
        )
        body = self.http_client.get_bytes(_url(self.config, f"/view?{query}"), timeout=30)
        return GenerationImage(
            filename=output.filename,
            mime_type=mimetypes.guess_type(output.filename)[0] or "application/octet-stream",
            data_base64=base64.b64encode(body).decode("ascii"),
        )


def parse_comfy_run_events(stdout: str) -> list[ComfyImageOutput]:
    node_outputs: list[ComfyImageOutput] = []
    terminal_outputs: list[ComfyImageOutput] = []
    completed = False
    for line in stdout.splitlines():
        if line.strip() == "":
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError as error:
            raise ComfyExecutionError("Comfy CLI emitted a malformed JSON event.") from error
        if not isinstance(event, dict):
            raise ComfyExecutionError("Comfy CLI emitted an unexpected JSON event.")

        event_type = event.get("event") or event.get("type")
        if event_type in {"execution_success", "completed", "success"}:
            completed = True
            terminal_outputs.extend(_event_images(event))
        else:
            node_outputs.extend(_event_images(event))

    if not completed:
        raise ComfyExecutionError("Comfy CLI did not report workflow completion.")
    outputs = terminal_outputs or node_outputs
    if not outputs:
        raise ComfyExecutionError("ComfyUI completed without image outputs.")
    return outputs


def _event_images(event: dict[str, Any]) -> list[ComfyImageOutput]:
    parsed = _output_images(event.get("outputs"))
    if parsed:
        return parsed

    output = _find_output(event)
    if not isinstance(output, dict):
        return []
    images = output.get("images")
    return _output_images(images)


def _output_images(images: Any) -> list[ComfyImageOutput]:
    if not isinstance(images, list):
        return []

    parsed: list[ComfyImageOutput] = []
    for image in images:
        if not isinstance(image, dict) or not isinstance(image.get("filename"), str):
            continue
        category = image.get("category")
        if isinstance(category, str) and category != "images":
            continue
        parsed.append(
            ComfyImageOutput(
                filename=image["filename"],
                subfolder=image.get("subfolder") if isinstance(image.get("subfolder"), str) else "",
                output_type=image.get("type") if isinstance(image.get("type"), str) else "output",
            )
        )
    return parsed


def _find_output(event: dict[str, Any]) -> Any:
    if "output" in event:
        return event.get("output")
    data = event.get("data")
    if isinstance(data, dict):
        return data.get("output")
    return None


def _url(config: EndpointConfig, path: str) -> str:
    return f"http://{_connect_host(config.comfyui_host)}:{config.comfyui_port}{path}"


def _connect_host(host: str) -> str:
    return "127.0.0.1" if host == "0.0.0.0" else host


def _write_extra_model_paths_config(config: EndpointConfig) -> Path:
    path = Path(tempfile.gettempdir()) / "luma-forge-comfy-extra-model-paths.yaml"
    workspace = config.workspace_mount_path
    path.write_text(
        "\n".join(
            [
                "luma_forge:",
                f"  base_path: {workspace}",
                "  checkpoints: models/checkpoints",
                "  text_encoders: models/text_encoders",
                "  vae: models/vae",
                "  loras: models/loras",
                "  diffusion_models: models/diffusion_models",
                "  clip: models/clip",
                "  clip_vision: models/clip_vision",
                "  controlnet: models/controlnet",
                "  upscale_models: models/upscale_models",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return path
