from dataclasses import dataclass
import os
from pathlib import Path


@dataclass(frozen=True)
class EndpointConfig:
    workspace_mount_path: Path = Path("/workspace")
    image_runtime_root_path: Path = Path("/opt/luma-forge/runtime")
    runtime_contract_id: str = "comfyui-python312-cu121"
    runtime_contract_version: str = "1.0.0"
    runtime_implementation_revision: str = "2026.05.16-001"
    endpoint_image_ref: str = "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222"
    comfyui_host: str = "127.0.0.1"
    comfyui_port: int = 8188
    comfyui_startup_timeout_seconds: float = 120
    max_prompt_chars: int = 4000
    generation_timeout_seconds: float = 300
    supported_execution_types: tuple[str, ...] = ("t2i",)
    workflow_relative_path: Path = Path("workflows/t2i.json")
    required_model_paths: tuple[Path, ...] = (Path("models/checkpoints/sd_xl_base_1.0.safetensors"),)
    required_custom_node_paths: tuple[Path, ...] = ()
    t2i_prompt_node_id: str | None = None
    t2i_prompt_input_key: str = "text"

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
            image_runtime_root_path=_absolute_path("LUMA_FORGE_IMAGE_RUNTIME_ROOT", "/opt/luma-forge/runtime"),
            runtime_contract_id=_string("LUMA_FORGE_RUNTIME_CONTRACT_ID", "comfyui-python312-cu121"),
            runtime_contract_version=_string("LUMA_FORGE_RUNTIME_CONTRACT_VERSION", "1.0.0"),
            runtime_implementation_revision=_string("LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION", "2026.05.16-001"),
            endpoint_image_ref=_string(
                "LUMA_FORGE_ENDPOINT_IMAGE_REF",
                "ghcr.io/luma-forge/runpod-endpoint-worker@sha256:2222222222222222222222222222222222222222222222222222222222222222",
            ),
            comfyui_host=_string("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_HOST", "127.0.0.1"),
            comfyui_port=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_PORT", 8188),
            comfyui_startup_timeout_seconds=_positive_float(
                "LUMA_FORGE_RUNPOD_ENDPOINT_COMFYUI_STARTUP_TIMEOUT_SECONDS",
                120,
            ),
            max_prompt_chars=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS", 4000),
            generation_timeout_seconds=_positive_float("LUMA_FORGE_RUNPOD_ENDPOINT_GENERATION_TIMEOUT_SECONDS", 300),
            supported_execution_types=tuple(_csv("LUMA_FORGE_RUNPOD_ENDPOINT_SUPPORTED_EXECUTION_TYPES", ("t2i",))),
            workflow_relative_path=Path(_string("LUMA_FORGE_RUNPOD_ENDPOINT_WORKFLOW_RELATIVE_PATH", "workflows/t2i.json")),
            required_model_paths=tuple(Path(value) for value in _csv(
                "LUMA_FORGE_RUNPOD_ENDPOINT_REQUIRED_MODEL_PATHS",
                ("models/checkpoints/sd_xl_base_1.0.safetensors",),
            )),
            required_custom_node_paths=tuple(
                Path(value) for value in _csv("LUMA_FORGE_RUNPOD_ENDPOINT_REQUIRED_CUSTOM_NODE_PATHS", ())
            ),
            t2i_prompt_node_id=_optional_string("LUMA_FORGE_RUNPOD_ENDPOINT_T2I_PROMPT_NODE_ID"),
            t2i_prompt_input_key=_string("LUMA_FORGE_RUNPOD_ENDPOINT_T2I_PROMPT_INPUT_KEY", "text"),
        )

    @property
    def comfyui_base_url(self) -> str:
        return f"http://{self.comfyui_host}:{self.comfyui_port}"

    @property
    def comfyui_root(self) -> Path:
        return self.image_runtime_root_path / "ComfyUI"

    @property
    def image_python_path(self) -> Path:
        return self.image_runtime_root_path / ".venv" / "bin" / "python"

    @property
    def image_runtime_contract_path(self) -> Path:
        return self.image_runtime_root_path / "runtime-contract.json"

    @property
    def runtime_manifest_path(self) -> Path:
        return self.workspace_mount_path / ".luma-forge" / "runtime-manifest.json"


def _string(name: str, default: str) -> str:
    value = os.environ.get(name)
    if value is None or value.strip() == "":
        return default
    return value.strip()


def _string_from(names: tuple[str, ...], default: str) -> str:
    for name in names:
        value = os.environ.get(name)
        if value is not None and value.strip() != "":
            return value.strip()
    return default


def _optional_string(name: str) -> str | None:
    value = os.environ.get(name)
    if value is None or value.strip() == "":
        return None
    return value.strip()


def _absolute_path(name: str, default: str) -> Path:
    value = _string(name, default)
    path = Path(value)
    if not path.is_absolute():
        return Path(default)
    return path.resolve(strict=False)


def _positive_int(name: str, default: int) -> int:
    try:
        value = int(os.environ.get(name, ""))
    except ValueError:
        return default
    return value if value > 0 else default


def _positive_float(name: str, default: float) -> float:
    try:
        value = float(os.environ.get(name, ""))
    except ValueError:
        return default
    return value if value > 0 else default


def _csv(name: str, default: tuple[str, ...]) -> list[str]:
    raw = os.environ.get(name)
    if raw is None:
        return list(default)
    return [part.strip() for part in raw.split(",") if part.strip()]
