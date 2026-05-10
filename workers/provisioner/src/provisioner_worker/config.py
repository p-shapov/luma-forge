from dataclasses import dataclass
import os


@dataclass(frozen=True)
class WorkerConfig:
    bearer_token: str | None = None
    max_request_bytes: int = 1024 * 1024
    git_timeout_seconds: float = 1800
    dependency_timeout_seconds: float = 1800
    download_timeout_seconds: float = 3600

    @classmethod
    def from_env(cls) -> "WorkerConfig":
        return cls(
            bearer_token=_optional_string("LUMA_FORGE_PROVISIONER_BEARER_TOKEN"),
            max_request_bytes=_positive_int("LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES", 1024 * 1024),
            git_timeout_seconds=_positive_float("LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS", 1800),
            dependency_timeout_seconds=_positive_float(
                "LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS",
                1800,
            ),
            download_timeout_seconds=_positive_float("LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS", 3600),
        )


def _optional_string(name: str) -> str | None:
    value = os.environ.get(name)
    if value is None or value.strip() == "":
        return None
    return value


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
