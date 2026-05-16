from collections.abc import Mapping
from dataclasses import dataclass
import ipaddress
import math
import os
from pathlib import Path
import re


DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8000
DEFAULT_MAX_REQUEST_BYTES = 1024 * 1024
DEFAULT_GIT_TIMEOUT_SECONDS = 1800.0
DEFAULT_DEPENDENCY_TIMEOUT_SECONDS = 1800.0
DEFAULT_DOWNLOAD_TIMEOUT_SECONDS = 3600.0
DEFAULT_WORKSPACE_MOUNT_PATH = "/workspace"

MAX_REQUEST_BYTES_LIMIT = 100 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 24 * 60 * 60
MIN_BEARER_TOKEN_LENGTH = 32

BEARER_TOKEN_ENV = "LUMA_FORGE_PROVISIONER_BEARER_TOKEN"
HOST_ENV = "LUMA_FORGE_PROVISIONER_HOST"
PORT_ENV = "LUMA_FORGE_PROVISIONER_PORT"
MAX_REQUEST_BYTES_ENV = "LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES"
GIT_TIMEOUT_ENV = "LUMA_FORGE_PROVISIONER_GIT_TIMEOUT_SECONDS"
DEPENDENCY_TIMEOUT_ENV = "LUMA_FORGE_PROVISIONER_DEPENDENCY_TIMEOUT_SECONDS"
DOWNLOAD_TIMEOUT_ENV = "LUMA_FORGE_PROVISIONER_DOWNLOAD_TIMEOUT_SECONDS"
WORKSPACE_MOUNT_PATH_ENV = "LUMA_FORGE_WORKSPACE_MOUNT_PATH"
RUNTIME_ARCHIVE_PATH_ENV = "LUMA_FORGE_RUNTIME_ARCHIVE_PATH"
RUNTIME_CONTRACT_ID_ENV = "LUMA_FORGE_RUNTIME_CONTRACT_ID"
RUNTIME_CONTRACT_VERSION_ENV = "LUMA_FORGE_RUNTIME_CONTRACT_VERSION"
RUNTIME_IMPLEMENTATION_REVISION_ENV = "LUMA_FORGE_RUNTIME_IMPLEMENTATION_REVISION"
PROVISIONER_IMAGE_REF_ENV = "LUMA_FORGE_PROVISIONER_IMAGE_REF"

DEFAULT_RUNTIME_ARCHIVE_PATH = "/opt/luma-forge/runtime/base-runtime.tar.zst"
DEFAULT_RUNTIME_CONTRACT_ID = "comfyui-python312-cu121"
DEFAULT_RUNTIME_CONTRACT_VERSION = "1.0.0"
DEFAULT_RUNTIME_IMPLEMENTATION_REVISION = "2026.05.16-001"

_DNS_LABEL = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$")


class ConfigurationError(Exception):
    code = "configuration_error"

    def __init__(self, env_name: str, reason_code: str, reason: str):
        super().__init__(f"Invalid Provisioner Worker configuration for {env_name}: {reason}.")
        self.env_name = env_name
        self.reason_code = reason_code
        self.reason = reason

    def to_dict(self) -> dict[str, str]:
        return {
            "code": self.code,
            "env_name": self.env_name,
            "reason_code": self.reason_code,
            "message": str(self),
        }


@dataclass(frozen=True)
class WorkerConfig:
    host: str
    port: int
    bearer_token: str
    max_request_bytes: int
    git_timeout_seconds: float
    dependency_timeout_seconds: float
    download_timeout_seconds: float
    workspace_mount_path: Path
    runtime_archive_path: Path
    runtime_contract_id: str
    runtime_contract_version: str
    runtime_implementation_revision: str
    provisioner_image_ref: str

    @classmethod
    def from_env(cls, env: Mapping[str, str] | None = None) -> "WorkerConfig":
        source = os.environ if env is None else env
        return cls(
            host=_parse_host(source),
            port=_parse_int(
                source,
                PORT_ENV,
                DEFAULT_PORT,
                minimum=1,
                maximum=65535,
            ),
            bearer_token=_parse_bearer_token(source),
            max_request_bytes=_parse_int(
                source,
                MAX_REQUEST_BYTES_ENV,
                DEFAULT_MAX_REQUEST_BYTES,
                minimum=1,
                maximum=MAX_REQUEST_BYTES_LIMIT,
            ),
            git_timeout_seconds=_parse_float(
                source,
                GIT_TIMEOUT_ENV,
                DEFAULT_GIT_TIMEOUT_SECONDS,
                minimum=0.0,
                maximum=MAX_TIMEOUT_SECONDS,
            ),
            dependency_timeout_seconds=_parse_float(
                source,
                DEPENDENCY_TIMEOUT_ENV,
                DEFAULT_DEPENDENCY_TIMEOUT_SECONDS,
                minimum=0.0,
                maximum=MAX_TIMEOUT_SECONDS,
            ),
            download_timeout_seconds=_parse_float(
                source,
                DOWNLOAD_TIMEOUT_ENV,
                DEFAULT_DOWNLOAD_TIMEOUT_SECONDS,
                minimum=0.0,
                maximum=MAX_TIMEOUT_SECONDS,
            ),
            workspace_mount_path=_parse_workspace_mount_path(source),
            runtime_archive_path=_parse_absolute_path(source, RUNTIME_ARCHIVE_PATH_ENV, DEFAULT_RUNTIME_ARCHIVE_PATH),
            runtime_contract_id=_configured_or_default(source, RUNTIME_CONTRACT_ID_ENV, DEFAULT_RUNTIME_CONTRACT_ID),
            runtime_contract_version=_configured_or_default(
                source,
                RUNTIME_CONTRACT_VERSION_ENV,
                DEFAULT_RUNTIME_CONTRACT_VERSION,
            ),
            runtime_implementation_revision=_configured_or_default(
                source,
                RUNTIME_IMPLEMENTATION_REVISION_ENV,
                DEFAULT_RUNTIME_IMPLEMENTATION_REVISION,
            ),
            provisioner_image_ref=_required_configured_value(source, PROVISIONER_IMAGE_REF_ENV),
        )


def _parse_bearer_token(env: Mapping[str, str]) -> str:
    raw = env.get(BEARER_TOKEN_ENV)
    if raw is None:
        raise ConfigurationError(BEARER_TOKEN_ENV, "missing_required_value", "value is required")
    token = raw.strip()
    if token == "":
        raise ConfigurationError(BEARER_TOKEN_ENV, "blank_value", "value must not be blank")
    if token != raw:
        raise ConfigurationError(
            BEARER_TOKEN_ENV,
            "surrounding_whitespace",
            "value must not contain surrounding whitespace",
        )
    if len(token) < MIN_BEARER_TOKEN_LENGTH:
        raise ConfigurationError(
            BEARER_TOKEN_ENV,
            "value_too_short",
            f"value must be at least {MIN_BEARER_TOKEN_LENGTH} characters",
        )
    if any(character.isspace() or ord(character) < 32 or ord(character) == 127 for character in token):
        raise ConfigurationError(
            BEARER_TOKEN_ENV,
            "invalid_characters",
            "value must not contain whitespace or control characters",
        )
    if not token.isascii():
        raise ConfigurationError(
            BEARER_TOKEN_ENV,
            "invalid_characters",
            "value must contain only ASCII characters",
        )
    return token


def _parse_host(env: Mapping[str, str]) -> str:
    host = _configured_or_default(env, HOST_ENV, DEFAULT_HOST)
    if _is_valid_ip_address(host) or _is_valid_dns_hostname(host):
        return host
    raise ConfigurationError(HOST_ENV, "invalid_host", "value must be a valid IP address or DNS hostname")


def _parse_workspace_mount_path(env: Mapping[str, str]) -> Path:
    return _parse_absolute_path(env, WORKSPACE_MOUNT_PATH_ENV, DEFAULT_WORKSPACE_MOUNT_PATH)


def _parse_absolute_path(env: Mapping[str, str], name: str, default: str) -> Path:
    raw = _configured_or_default(env, name, default)
    path = Path(raw)
    if not path.is_absolute():
        raise ConfigurationError(name, "path_not_absolute", "value must be an absolute path")
    if str(path) != raw or any(part in ("", ".", "..") for part in path.parts):
        raise ConfigurationError(name, "path_not_normalized", "value must be normalized")
    return path.resolve(strict=False)


def _parse_int(
    env: Mapping[str, str],
    name: str,
    default: int,
    *,
    minimum: int,
    maximum: int,
) -> int:
    raw = env.get(name)
    if raw is None:
        return default
    value = raw.strip()
    if value == "":
        raise ConfigurationError(name, "blank_value", "value must not be blank")
    try:
        parsed = int(value, 10)
    except ValueError as error:
        raise ConfigurationError(name, "invalid_integer", "value must be an integer") from error
    if parsed < minimum or parsed > maximum:
        raise ConfigurationError(name, "integer_out_of_range", f"value must be between {minimum} and {maximum}")
    return parsed


def _parse_float(
    env: Mapping[str, str],
    name: str,
    default: float,
    *,
    minimum: float,
    maximum: float,
) -> float:
    raw = env.get(name)
    if raw is None:
        return default
    value = raw.strip()
    if value == "":
        raise ConfigurationError(name, "blank_value", "value must not be blank")
    try:
        parsed = float(value)
    except ValueError as error:
        raise ConfigurationError(name, "invalid_number", "value must be a number") from error
    if not math.isfinite(parsed) or parsed <= minimum or parsed > maximum:
        raise ConfigurationError(
            name,
            "number_out_of_range",
            f"value must be greater than {minimum:g} and at most {maximum:g}",
        )
    return parsed


def _configured_or_default(env: Mapping[str, str], name: str, default: str) -> str:
    raw = env.get(name)
    if raw is None:
        return default
    return _non_blank_configured_value(name, raw)


def _required_configured_value(env: Mapping[str, str], name: str) -> str:
    raw = env.get(name)
    if raw is None:
        raise ConfigurationError(name, "missing_required_value", "value is required")
    return _non_blank_configured_value(name, raw)


def _non_blank_configured_value(name: str, raw: str) -> str:
    value = raw.strip()
    if value == "":
        raise ConfigurationError(name, "blank_value", "value must not be blank")
    return value


def _is_valid_ip_address(value: str) -> bool:
    try:
        ipaddress.ip_address(value)
    except ValueError:
        return False
    return True


def _is_valid_dns_hostname(value: str) -> bool:
    if len(value) > 253:
        return False
    hostname = value[:-1] if value.endswith(".") else value
    if not hostname:
        return False
    return all(_DNS_LABEL.fullmatch(label) is not None for label in hostname.split("."))
