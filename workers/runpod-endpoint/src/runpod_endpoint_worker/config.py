from dataclasses import dataclass
import os
from pathlib import Path


@dataclass(frozen=True)
class EndpointConfig:
    workspace_mount_path: Path = Path("/workspace")
    comfy_cli_path: Path = Path("/opt/luma-forge/runtime/.venv/bin/comfy")
    comfyui_path: Path = Path("/opt/luma-forge/runtime/ComfyUI")
    workflow_path: Path = Path("/opt/luma-forge/runtime/workflows/workflow.json")
    comfyui_host: str = "127.0.0.1"
    comfyui_port: int = 8188
    comfyui_startup_timeout_seconds: int = 300
    comfy_ui_ready_poll_seconds: float = 1.0
    execution_timeout_seconds: int = 900
    max_prompt_chars: int = 4000
    supported_execution_types: tuple[str, ...] = ("t2i",)

    @classmethod
    def from_env(cls) -> "EndpointConfig":
        return cls(
            workspace_mount_path=Path(_string_from(
                (
                    "LUMA_FORGE_RUNPOD_ENDPOINT_WORKSPACE_MOUNT_PATH",
                    "LUMA_FORGE_WORKSPACE_MOUNT_PATH",
                ),
                "/workspace",
            )),
            comfy_cli_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_COMFY_CLI_PATH", "/opt/luma-forge/runtime/.venv/bin/comfy")),
            comfyui_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PATH", "/opt/luma-forge/runtime/ComfyUI")),
            workflow_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_WORKFLOW_PATH", "/opt/luma-forge/runtime/workflows/workflow.json")),
            comfyui_host=_string("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST", "127.0.0.1"),
            comfyui_port=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT", 8188),
            comfyui_startup_timeout_seconds=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS", 300),
            execution_timeout_seconds=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_TIMEOUT_SECONDS", 900),
            max_prompt_chars=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS", 4000),
            supported_execution_types=tuple(_csv("LUMA_FORGE_RUNPOD_ENDPOINT_SUPPORTED_EXECUTION_TYPES", ("t2i",))),
        )


def _string_from(names: tuple[str, ...], default: str) -> str:
    for name in names:
        value = os.environ.get(name)
        if value is not None and value.strip() != "":
            return value.strip()
    return default


def _string(name: str, default: str) -> str:
    value = os.environ.get(name)
    if value is None or value.strip() == "":
        return default
    return value.strip()


def _positive_int(name: str, default: int) -> int:
    try:
        value = int(os.environ.get(name, ""))
    except ValueError:
        return default
    return value if value > 0 else default


def _csv(name: str, default: tuple[str, ...]) -> list[str]:
    raw = os.environ.get(name)
    if raw is None:
        return list(default)
    return [part.strip() for part in raw.split(",") if part.strip()]
