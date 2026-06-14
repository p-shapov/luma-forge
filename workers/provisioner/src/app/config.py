from collections.abc import Mapping
from dataclasses import dataclass
import json
import os
from pathlib import Path

from app.schemas import StartRequest, parse_start_request
from app.errors import ValidationError

HOST = "0.0.0.0"
PORT = 8000
DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS = 3600.0
WORKSPACE_MOUNT_PATH = "/workspace"

BEARER_TOKEN_ENV = "LUMA_FORGE_PROVISIONER_BEARER_TOKEN"
REQUIRED_MODEL_ASSETS_ENV = "LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS"
HUGGING_FACE_API_KEY_ENV = "LUMA_FORGE_HUGGING_FACE_API_KEY"


class ConfigurationError(Exception):
    def __init__(self, env_name: str, code: str, reason: str):
        super().__init__(f"Invalid Provisioner Worker configuration for {env_name}: {reason}.")
        self.env_name = env_name
        self.code = code
        self.reason = reason

    def to_dict(self) -> dict[str, str]:
        return {
            "code": self.code,
            "env_name": self.env_name,
            "message": str(self),
        }


@dataclass(frozen=True)
class WorkerConfig:
    host: str
    port: int
    bearer_token: str
    start_request: StartRequest
    download_inactivity_timeout_seconds: float
    workspace_mount_path: Path
    hugging_face_api_key: str | None

    @classmethod
    def from_env(cls, env: Mapping[str, str] | None = None) -> "WorkerConfig":
        source = os.environ if env is None else env
        return cls(
            host=HOST,
            port=PORT,
            bearer_token=_parse_bearer_token(source),
            start_request=_parse_start_request(source),
            download_inactivity_timeout_seconds=DOWNLOAD_INACTIVITY_TIMEOUT_SECONDS,
            workspace_mount_path=Path(WORKSPACE_MOUNT_PATH).resolve(strict=False),
            hugging_face_api_key=_parse_optional_secret(source, HUGGING_FACE_API_KEY_ENV),
        )


def _parse_bearer_token(env: Mapping[str, str]) -> str:
    raw = env.get(BEARER_TOKEN_ENV)
    if raw is None:
        raise ConfigurationError(BEARER_TOKEN_ENV, "missing_required_value", "value is required")
    if raw == "":
        raise ConfigurationError(BEARER_TOKEN_ENV, "blank_value", "value must not be blank")
    return raw


def _parse_start_request(env: Mapping[str, str]) -> StartRequest:
    payload = {
        "required_model_assets": _parse_required_model_assets(env),
    }
    try:
        return parse_start_request(payload)
    except ValidationError as error:
        raise ConfigurationError(REQUIRED_MODEL_ASSETS_ENV, "invalid_request", error.message) from error


def _parse_optional_secret(env: Mapping[str, str], name: str) -> str | None:
    raw = env.get(name)
    if raw is None:
        return None
    value = raw.strip()
    return value or None


def _parse_required_model_assets(env: Mapping[str, str]) -> list[dict[str, object]]:
    raw = _required_configured_value(env, REQUIRED_MODEL_ASSETS_ENV)
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ConfigurationError(
            REQUIRED_MODEL_ASSETS_ENV,
            "invalid_json",
            "value must be valid JSON",
        ) from error
    if not isinstance(payload, list):
        raise ConfigurationError(
            REQUIRED_MODEL_ASSETS_ENV,
            "invalid_request",
            "value must be an array",
        )
    return payload


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
