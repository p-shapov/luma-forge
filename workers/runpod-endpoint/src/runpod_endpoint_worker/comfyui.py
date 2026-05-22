from collections.abc import Callable
from dataclasses import dataclass, field
import base64
import atexit
import copy
import json
import os
from pathlib import Path
import subprocess
from threading import Lock
from time import monotonic, sleep
from typing import Any, Protocol
from urllib import parse, request
from urllib.error import URLError

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.environment import PreparedRuntimeManifest, workflow_path
from runpod_endpoint_worker.errors import (
    ComfyUiExecutionError,
    ComfyUiStartupError,
    ComfyUiTimeoutError,
    ValidationError,
)
from runpod_endpoint_worker.schemas import GenerationRequest, ImageOutput

JsonTransport = Callable[[str, str, dict[str, Any] | None, float], dict[str, Any]]
BytesTransport = Callable[[str, float], bytes]
ProcessFactory = Callable[[list[str], Path, dict[str, str]], "ProcessLike"]


class ProcessLike(Protocol):
    def poll(self) -> int | None: ...

    def terminate(self) -> None: ...

    def kill(self) -> None: ...

    def wait(self, timeout: float | None = None) -> int: ...


def _start_comfyui_process(command: list[str], cwd: Path, env: dict[str, str]) -> ProcessLike:
    return subprocess.Popen(command, cwd=cwd, env={**os.environ, **env})


@dataclass(frozen=True)
class ComfyUiClient:
    base_url: str
    timeout_seconds: float
    json_transport: JsonTransport | None = None
    bytes_transport: BytesTransport | None = None

    def assert_available(self) -> None:
        try:
            self._json("GET", "/system_stats")
        except Exception as error:
            raise ComfyUiStartupError("ComfyUI is unavailable.") from error

    def queue_prompt(self, workflow: dict[str, Any]) -> str:
        payload = self._json("POST", "/prompt", {"prompt": workflow})
        prompt_id = payload.get("prompt_id")
        if not isinstance(prompt_id, str) or prompt_id.strip() == "":
            raise ComfyUiExecutionError("ComfyUI did not return a prompt id.")
        return prompt_id

    def wait_for_image(self, prompt_id: str) -> ImageOutput:
        deadline = monotonic() + self.timeout_seconds
        while monotonic() < deadline:
            history = self._json("GET", f"/history/{parse.quote(prompt_id)}")
            image_ref = _first_image_ref(history, prompt_id)
            if image_ref is not None:
                return self._download_image(image_ref)
            sleep(0.25)

        raise ComfyUiTimeoutError("ComfyUI generation timed out.")

    def _download_image(self, image_ref: dict[str, str]) -> ImageOutput:
        query = parse.urlencode(image_ref)
        image_bytes = self._bytes(f"/view?{query}")
        return ImageOutput(
            mime_type=_mime_type(image_ref["filename"]),
            data=base64.b64encode(image_bytes).decode("ascii"),
        )

    def _json(self, method: str, path: str, payload: dict[str, Any] | None = None) -> dict[str, Any]:
        if self.json_transport is not None:
            return self.json_transport(method, path, payload, self.timeout_seconds)

        body = None if payload is None else json.dumps(payload).encode("utf-8")
        headers = {"Content-Type": "application/json"} if payload is not None else {}
        req = request.Request(f"{self.base_url}{path}", data=body, headers=headers, method=method)
        try:
            with request.urlopen(req, timeout=self.timeout_seconds) as response:
                return json.loads(response.read().decode("utf-8"))
        except (OSError, URLError, json.JSONDecodeError) as error:
            raise ComfyUiExecutionError("ComfyUI request failed.") from error

    def _bytes(self, path: str) -> bytes:
        if self.bytes_transport is not None:
            return self.bytes_transport(path, self.timeout_seconds)

        try:
            with request.urlopen(f"{self.base_url}{path}", timeout=self.timeout_seconds) as response:
                return response.read()
        except OSError as error:
            raise ComfyUiExecutionError("ComfyUI image download failed.") from error


@dataclass
class ComfyUiProcessManager:
    config: EndpointConfig
    client: ComfyUiClient
    process_factory: ProcessFactory = _start_comfyui_process
    clock: Callable[[], float] = monotonic
    sleeper: Callable[[float], None] = sleep
    poll_interval_seconds: float = 0.25
    _process: ProcessLike | None = field(default=None, init=False, repr=False)
    _lock: Lock = field(default_factory=Lock, init=False, repr=False)
    _shutdown_registered: bool = field(default=False, init=False, repr=False)

    def ensure_running(self, runtime: PreparedRuntimeManifest | None = None) -> None:
        with self._lock:
            if self._is_http_ready():
                return

            if runtime is None:
                raise ComfyUiStartupError("Prepared runtime manifest is required.")

            if self._process is None or self._process.poll() is not None:
                self._process = self.process_factory(
                    [
                        str(self.config.image_python_path),
                        str(self.config.comfyui_root / "main.py"),
                        "--base-directory",
                        str(runtime.workspace_root),
                        "--output-directory",
                        str(runtime.workspace_root / "output"),
                        "--listen",
                        self.config.comfyui_host,
                        "--port",
                        str(self.config.comfyui_port),
                    ],
                    self.config.comfyui_root,
                    _runtime_env(runtime),
                )
                self._register_shutdown()

            self._wait_until_ready()

    def shutdown(self) -> None:
        with self._lock:
            process = self._process
            self._process = None

        if process is None or process.poll() is not None:
            return

        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)

    def _wait_until_ready(self) -> None:
        deadline = self.clock() + self.config.comfyui_startup_timeout_seconds

        while self.clock() < deadline:
            if self._process is not None and self._process.poll() is not None:
                raise ComfyUiStartupError("ComfyUI exited before becoming ready.")
            if self._is_http_ready():
                return
            self.sleeper(self.poll_interval_seconds)

        raise ComfyUiStartupError("ComfyUI did not become ready before startup timeout.")

    def _is_http_ready(self) -> bool:
        try:
            self.client.assert_available()
            return True
        except ComfyUiStartupError:
            return False

    def _register_shutdown(self) -> None:
        if self._shutdown_registered:
            return
        atexit.register(self.shutdown)
        self._shutdown_registered = True


def render_t2i_workflow(
    config: EndpointConfig,
    generation_request: GenerationRequest,
    runtime: PreparedRuntimeManifest | None = None,
) -> dict[str, Any]:
    runtime = runtime or validate_runtime_for_workflow(config)
    with workflow_path(config, runtime).open("r", encoding="utf-8") as file:
        workflow = json.load(file)

    rendered = copy.deepcopy(workflow)
    if config.t2i_prompt_node_id is not None:
        _set_prompt_node(rendered, config.t2i_prompt_node_id, config.t2i_prompt_input_key, generation_request.prompt)
        return rendered

    replaced = _replace_prompt_placeholder(rendered, generation_request.prompt)
    if not replaced:
        raise ValidationError("workflow prompt placeholder is missing")
    return rendered


def validate_runtime_for_workflow(config: EndpointConfig) -> PreparedRuntimeManifest:
    from runpod_endpoint_worker.environment import validate_prepared_environment

    return validate_prepared_environment(config)


def _runtime_env(runtime: PreparedRuntimeManifest) -> dict[str, str]:
    _ = runtime
    return {
        "LUMA_FORGE_WORKSPACE_ROOT": str(runtime.workspace_root),
        "LUMA_FORGE_MODELS_ROOT": str(runtime.workspace_root / "models"),
        "LUMA_FORGE_OUTPUT_ROOT": str(runtime.workspace_root / "output"),
    }


def _set_prompt_node(workflow: dict[str, Any], node_id: str, input_key: str, prompt: str) -> None:
    node = workflow.get(node_id)
    if not isinstance(node, dict):
        raise ValidationError("configured prompt node is missing")
    inputs = node.get("inputs")
    if not isinstance(inputs, dict):
        raise ValidationError("configured prompt node inputs are missing")
    inputs[input_key] = prompt


def _replace_prompt_placeholder(value: Any, prompt: str) -> bool:
    if isinstance(value, dict):
        replaced = False
        for key, child in value.items():
            if child == "{{prompt}}":
                value[key] = prompt
                replaced = True
            else:
                replaced = _replace_prompt_placeholder(child, prompt) or replaced
        return replaced
    if isinstance(value, list):
        replaced = False
        for index, child in enumerate(value):
            if child == "{{prompt}}":
                value[index] = prompt
                replaced = True
            else:
                replaced = _replace_prompt_placeholder(child, prompt) or replaced
        return replaced
    return False


def _first_image_ref(history: dict[str, Any], prompt_id: str) -> dict[str, str] | None:
    prompt_history = history.get(prompt_id) if prompt_id in history else history
    if not isinstance(prompt_history, dict):
        return None
    outputs = prompt_history.get("outputs")
    if not isinstance(outputs, dict):
        return None

    for output in outputs.values():
        if not isinstance(output, dict):
            continue
        images = output.get("images")
        if not isinstance(images, list) or not images:
            continue
        image = images[0]
        if not isinstance(image, dict):
            continue
        filename = image.get("filename")
        if isinstance(filename, str) and filename.strip():
            return {
                "filename": filename,
                "subfolder": image.get("subfolder", ""),
                "type": image.get("type", "output"),
            }
    return None


def _mime_type(filename: str) -> str:
    suffix = Path(filename).suffix.lower()
    if suffix in (".jpg", ".jpeg"):
        return "image/jpeg"
    if suffix == ".webp":
        return "image/webp"
    return "image/png"
