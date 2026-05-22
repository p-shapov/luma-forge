from dataclasses import dataclass
import os
from pathlib import Path


@dataclass(frozen=True)
class EndpointConfig:
    workspace_mount_path: Path = Path("/workspace")
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
            max_prompt_chars=_positive_int("LUMA_FORGE_RUNPOD_ENDPOINT_MAX_PROMPT_CHARS", 4000),
            supported_execution_types=tuple(_csv("LUMA_FORGE_RUNPOD_ENDPOINT_SUPPORTED_EXECUTION_TYPES", ("t2i",))),
        )


def _string_from(names: tuple[str, ...], default: str) -> str:
    for name in names:
        value = os.environ.get(name)
        if value is not None and value.strip() != "":
            return value.strip()
    return default


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
