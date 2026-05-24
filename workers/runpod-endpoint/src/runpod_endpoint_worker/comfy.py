from __future__ import annotations

import base64
import json
import mimetypes
import re
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
from runpod_endpoint_worker.errors import (
    ComfyLaunchError,
    ComfyNoOutputsError,
    ComfyOutputFetchError,
    ComfyOutputParseError,
    ComfyStartupTimeoutError,
    ComfyWorkflowError,
    ComfyWorkflowTimeoutError,
    ResponseTooLargeError,
    safe_error_message,
)
from runpod_endpoint_worker.logging import LOGGER
from runpod_endpoint_worker.schemas import GenerationImage, GenerationRequest
from runpod_endpoint_worker.workflow import write_patched_workflow


_COMFY_STARTUP_LOCK = threading.Lock()
_DIAGNOSTIC_EXCERPT_MAX_CHARS = 600
_SIGNED_URL_PATTERN = re.compile(
    r"(https?://[^\s?]+)\?[^\s]*(?:X-Amz-Signature|Signature=|AWSAccessKeyId|X-Amz-Credential|Expires=)[^\s]*"
)
_URL_USERINFO_PATTERN = re.compile(r"(https?://)[^/\s:@]+:[^/\s@]+@")
_AUTH_HEADER_PATTERN = re.compile(r"(?i)(authorization\s*:\s*)[^\r\n]+")
_KEY_VALUE_SECRET_PATTERN = re.compile(
    r"(?i)\b([A-Z0-9_-]*(?:api[_-]?key|access[_-]?key|secret|password|credential))\b\s*[:=]\s*[^\s,;]+"
)
_HUGGING_FACE_TOKEN_PATTERN = re.compile(r"\bhf_[A-Za-z0-9_=-]{20,}\b")
_COMMAND_INVOCATION_PATTERN = re.compile(r"(?im)^\s*(?:command|cmd|argv|args)\s*[:=]\s*\S+")
_ENVIRONMENT_DUMP_PATTERN = re.compile(
    r"(?m)(?:^|\n)\s*[A-Za-z_][A-Za-z0-9_]*=[^\n]*(?:\n\s*[A-Za-z_][A-Za-z0-9_]*=[^\n]*)+"
)


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

            raise ComfyStartupTimeoutError("ComfyUI did not become ready before the startup timeout.")

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
        except subprocess.TimeoutExpired as error:
            _log_process_output("ComfyUI launch subprocess timed out", error)
            raise ComfyStartupTimeoutError(
                _process_failure_message("ComfyUI startup timed out.", error),
                metadata=_process_failure_metadata(error),
            ) from error
        except (OSError, subprocess.SubprocessError) as error:
            _log_process_output("ComfyUI launch subprocess failed", error)
            raise ComfyLaunchError(
                _process_failure_message("ComfyUI failed to launch.", error),
                metadata=_process_failure_metadata(error),
            ) from error

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
            raise ComfyNoOutputsError("ComfyUI completed without image outputs.")

        images = [self._fetch_image(output) for output in outputs]
        if sum(len(image.data_base64) for image in images) > self.config.max_response_bytes:
            raise ResponseTooLargeError("Generated image response exceeds the inline response size limit.")
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
        except subprocess.TimeoutExpired as error:
            _log_process_output("ComfyUI workflow subprocess timed out", error)
            raise ComfyWorkflowTimeoutError(
                _process_failure_message("ComfyUI workflow execution timed out.", error),
                metadata=_process_failure_metadata(error),
            ) from error
        except (OSError, subprocess.SubprocessError) as error:
            _log_process_output("ComfyUI workflow subprocess failed", error)
            raise ComfyWorkflowError(
                _process_failure_message("ComfyUI workflow execution failed.", error),
                metadata=_process_failure_metadata(error),
            ) from error
        return parse_comfy_run_events(completed.stdout)

    def _fetch_image(self, output: ComfyImageOutput) -> GenerationImage:
        query = urlencode(
            {
                "filename": output.filename,
                "subfolder": output.subfolder,
                "type": output.output_type,
            }
        )
        try:
            body = self.http_client.get_bytes(_url(self.config, f"/view?{query}"), timeout=30)
        except Exception as error:
            raise ComfyOutputFetchError("ComfyUI generated output could not be fetched.") from error
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
            raise ComfyOutputParseError("Comfy CLI emitted a malformed JSON event.") from error
        if not isinstance(event, dict):
            raise ComfyOutputParseError("Comfy CLI emitted an unexpected JSON event.")

        event_type = event.get("event") or event.get("type")
        if event_type in {"execution_success", "completed", "success"}:
            completed = True
            terminal_outputs.extend(_event_images(event))
        else:
            node_outputs.extend(_event_images(event))

    if not completed:
        raise ComfyWorkflowError("Comfy CLI did not report workflow completion.")
    outputs = terminal_outputs or node_outputs
    if not outputs:
        raise ComfyNoOutputsError("ComfyUI completed without image outputs.")
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


def _process_failure_message(prefix: str, error: BaseException) -> str:
    if isinstance(error, subprocess.TimeoutExpired):
        if error.timeout is None:
            return f"{prefix} Timed out."
        return f"{prefix} Timed out after {error.timeout:g} seconds."
    if isinstance(error, subprocess.CalledProcessError):
        return f"{prefix} Process exited with status {error.returncode}."
    return prefix


def _process_failure_metadata(error: BaseException) -> dict[str, object]:
    metadata: dict[str, object] = {}
    if isinstance(error, subprocess.TimeoutExpired) and error.timeout is not None:
        metadata["timeout_seconds"] = error.timeout
    if isinstance(error, subprocess.CalledProcessError):
        metadata["exit_status"] = error.returncode
    diagnostic_excerpt = _diagnostic_excerpt(error)
    if diagnostic_excerpt:
        metadata["diagnostic_excerpt"] = diagnostic_excerpt
    metadata.update(_comfy_json_error_metadata(error))
    return metadata


def _diagnostic_excerpt(error: BaseException) -> str | None:
    output = " ".join(_process_output(error).split())
    if output == "":
        return None
    comfy_error = _comfy_json_error_payload(error)
    if comfy_error is not None:
        summary = _comfy_json_error_summary(comfy_error)
        if summary:
            return summary
    if len(output) > _DIAGNOSTIC_EXCERPT_MAX_CHARS:
        marker = "Error log during ComfyUI execution"
        marker_index = output.find(marker)
        if marker_index >= 0:
            output = output[marker_index:]
            if len(output) <= _DIAGNOSTIC_EXCERPT_MAX_CHARS:
                return output
        return f"...{output[-(_DIAGNOSTIC_EXCERPT_MAX_CHARS - 3):]}"
    return output


def _process_output(error: BaseException) -> str:
    parts: list[str] = []
    for value in (getattr(error, "stderr", None), getattr(error, "stdout", None)):
        if isinstance(value, bytes):
            parts.append(value.decode("utf-8", errors="replace"))
        elif isinstance(value, str):
            parts.append(value)
    return "\n".join(parts)


def _comfy_json_error_metadata(error: BaseException) -> dict[str, object]:
    error_payload = _comfy_json_error_payload(error)
    if error_payload is None:
        return {}
    metadata: dict[str, object] = {}
    _copy_str_metadata(error_payload, metadata, "kind", "comfy_error_kind")
    _copy_str_metadata(error_payload, metadata, "message", "comfy_error_message")
    _copy_str_metadata(error_payload, metadata, "node_id", "comfy_node_id")
    _copy_str_metadata(error_payload, metadata, "class_type", "comfy_class_type")
    _copy_str_metadata(error_payload, metadata, "exception_type", "comfy_exception_type")
    status_code = error_payload.get("status_code")
    if isinstance(status_code, int) and not isinstance(status_code, bool):
        metadata["comfy_status_code"] = status_code
    return metadata


def _comfy_json_error_payload(error: BaseException) -> dict[str, Any] | None:
    output = getattr(error, "output", None)
    if isinstance(output, bytes):
        stdout = output.decode("utf-8", errors="replace")
    elif isinstance(output, str):
        stdout = output
    else:
        return None

    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(event, dict) or event.get("event") != "failed":
            continue
        error_payload = event.get("error")
        if not isinstance(error_payload, dict):
            continue
        return error_payload
    return None


def _comfy_json_error_summary(error_payload: dict[str, Any]) -> str | None:
    kind = error_payload.get("kind")
    message = error_payload.get("message")
    if isinstance(kind, str) and kind.strip() != "" and isinstance(message, str) and message.strip() != "":
        return f"{kind}: {message}"
    if isinstance(message, str) and message.strip() != "":
        return message
    if isinstance(kind, str) and kind.strip() != "":
        return kind
    return None


def _copy_str_metadata(source: dict[str, Any], target: dict[str, object], source_key: str, target_key: str) -> None:
    value = source.get(source_key)
    if isinstance(value, str) and value.strip() != "":
        target[target_key] = value


def _log_process_output(message: str, error: BaseException) -> None:
    output = _process_output(error)
    if output:
        LOGGER.warning("%s subprocess_output=%s", message, _safe_process_log_output(output))


def _safe_process_log_output(output: str) -> str:
    scrubbed = _scrub_process_output(output)
    if _contains_disallowed_log_shape(scrubbed) or safe_error_message(scrubbed) == "Endpoint worker request failed.":
        return "redacted"
    return scrubbed


def _scrub_process_output(output: str) -> str:
    scrubbed = _SIGNED_URL_PATTERN.sub(r"\1?<redacted:signed-query>", output)
    scrubbed = _URL_USERINFO_PATTERN.sub(r"\1<redacted:userinfo>@", scrubbed)
    scrubbed = _AUTH_HEADER_PATTERN.sub(r"\1<redacted:authorization>", scrubbed)
    scrubbed = _KEY_VALUE_SECRET_PATTERN.sub(r"\1=<redacted:value>", scrubbed)
    return _HUGGING_FACE_TOKEN_PATTERN.sub("<redacted:hf>", scrubbed)


def _contains_disallowed_log_shape(output: str) -> bool:
    return _COMMAND_INVOCATION_PATTERN.search(output) is not None or _ENVIRONMENT_DUMP_PATTERN.search(output) is not None


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
