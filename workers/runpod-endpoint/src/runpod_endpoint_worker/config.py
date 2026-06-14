from dataclasses import dataclass
from pathlib import Path

WORKSPACE_MOUNT_PATH = Path("/workspace")
COMFY_CLI_PATH = Path("/opt/luma-forge/runtime/.venv/bin/comfy")
COMFYUI_PATH = Path("/opt/luma-forge/runtime/ComfyUI")
WORKFLOW_PATH = Path("/opt/luma-forge/runtime/workflows/workflow.json")
EXECUTION_CONTRACT_PATH = Path("/opt/luma-forge/runtime/contracts/execution-contract.json")
COMFYUI_HOST = "127.0.0.1"
COMFYUI_PORT = 8188
COMFYUI_STARTUP_TIMEOUT_SECONDS = 300
COMFY_UI_READY_POLL_SECONDS = 1.0
EXECUTION_TIMEOUT_SECONDS = 900
MAX_RESPONSE_BYTES = 9_000_000
MAX_ARTIFACT_BYTES = 512_000_000


@dataclass(frozen=True)
class EndpointConfig:
    workspace_mount_path: Path = WORKSPACE_MOUNT_PATH
    comfy_cli_path: Path = COMFY_CLI_PATH
    comfyui_path: Path = COMFYUI_PATH
    workflow_path: Path = WORKFLOW_PATH
    execution_contract_path: Path = EXECUTION_CONTRACT_PATH
    comfyui_host: str = COMFYUI_HOST
    comfyui_port: int = COMFYUI_PORT
    comfyui_startup_timeout_seconds: int = COMFYUI_STARTUP_TIMEOUT_SECONDS
    comfy_ui_ready_poll_seconds: float = COMFY_UI_READY_POLL_SECONDS
    execution_timeout_seconds: int = EXECUTION_TIMEOUT_SECONDS
    max_response_bytes: int = MAX_RESPONSE_BYTES
    max_artifact_bytes: int = MAX_ARTIFACT_BYTES

    @classmethod
    def from_env(cls) -> "EndpointConfig":
        return cls()
