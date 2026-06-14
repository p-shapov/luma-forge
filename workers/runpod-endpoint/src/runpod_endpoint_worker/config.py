from dataclasses import dataclass
import os
from pathlib import Path


@dataclass(frozen=True)
class EndpointConfig:
    workspace_mount_path: Path = Path("/workspace")
    comfy_cli_path: Path = Path("/opt/luma-forge/runtime/.venv/bin/comfy")
    comfyui_path: Path = Path("/opt/luma-forge/runtime/ComfyUI")
    workflow_path: Path = Path("/opt/luma-forge/runtime/workflows/workflow.json")
    execution_contract_path: Path = Path("/opt/luma-forge/runtime/contracts/execution-contract.json")
    execution_schema_path: Path = Path("/opt/luma-forge/runtime/contracts/execution-schema.json")
    comfyui_host: str = "127.0.0.1"
    comfyui_port: int = 8188
    comfyui_startup_timeout_seconds: int = 300
    comfy_ui_ready_poll_seconds: float = 1.0
    execution_timeout_seconds: int = 900
    max_response_bytes: int = 9_000_000
    max_artifact_bytes: int = 512_000_000
    max_prompt_chars: int = 4000

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
            execution_contract_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_CONTRACT_PATH", "/opt/luma-forge/runtime/contracts/execution-contract.json")),
            execution_schema_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_SCHEMA_PATH", "/opt/luma-forge/runtime/contracts/execution-schema.json")),
            comfyui_host=_string("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST", "127.0.0.1"),
            comfyui_port=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT", 8188),
            comfyui_startup_timeout_seconds=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS", 300),
            execution_timeout_seconds=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_EXECUTION_TIMEOUT_SECONDS", 900),
            max_response_bytes=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_RESPONSE_BYTES", 9_000_000),
            max_artifact_bytes=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_ARTIFACT_BYTES", 512_000_000),
            max_prompt_chars=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS", 4000),
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
