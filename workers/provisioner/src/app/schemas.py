from dataclasses import dataclass
from pathlib import Path
import re
from typing import Any

from app.errors import ValidationError
from auxiliary.paths import safe_relative_path

MAX_IDENTIFIER_LENGTH = 128
MAX_DISPLAY_NAME_LENGTH = 256
SAFE_IDENTIFIER_PATTERN = re.compile(r"[A-Za-z0-9._-]+")


@dataclass(frozen=True)
class HuggingFaceSource:
    repository_id: str
    file_path: str
    revision: str


@dataclass(frozen=True)
class ModelAsset:
    id: str
    name: str
    download_source: HuggingFaceSource
    install_comfyui_relative_path: Path


@dataclass(frozen=True)
class WorkflowPreset:
    required_model_assets: list[ModelAsset]


@dataclass(frozen=True)
class StartRequest:
    job_id: str
    workflow_preset: WorkflowPreset


def parse_start_request(payload: Any) -> StartRequest:
    data = _object(payload, "request")
    _require_keys(data, {"job_id", "workflow_preset"}, "request")
    return StartRequest(
        job_id=_non_empty_string(data.get("job_id"), "job_id"),
        workflow_preset=_parse_workflow_preset(data.get("workflow_preset")),
    )


def _parse_workflow_preset(payload: Any) -> WorkflowPreset:
    data = _object(payload, "workflow_preset")
    _require_keys(
        data,
        {"required_model_assets"},
        "workflow_preset",
    )
    return WorkflowPreset(
        required_model_assets=[
            _parse_model_asset(item, f"workflow_preset.required_model_assets[{index}]")
            for index, item in enumerate(
                _list(data.get("required_model_assets"), "workflow_preset.required_model_assets")
            )
        ],
    )


def _parse_huggingface_source(payload: Any, field: str) -> HuggingFaceSource:
    data = _object(payload, field)
    _require_keys(data, {"source_type", "repository_id", "file_path", "revision"}, field)
    source_type = _non_empty_string(data.get("source_type"), f"{field}.source_type")
    if source_type != "huggingface":
        raise ValidationError(f"{field}.source_type must be huggingface")
    return HuggingFaceSource(
        repository_id=_non_empty_string(data.get("repository_id"), f"{field}.repository_id"),
        file_path=safe_relative_path(data.get("file_path"), field_name=f"{field}.file_path").as_posix(),
        revision=_non_empty_string(data.get("revision"), f"{field}.revision"),
    )


def _parse_model_asset(payload: Any, field: str) -> ModelAsset:
    data = _object(payload, field)
    _require_keys(data, {"id", "name", "download_source", "install_comfyui_relative_path"}, field)
    return ModelAsset(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        name=_display_name(data.get("name"), f"{field}.name"),
        download_source=_parse_huggingface_source(data.get("download_source"), f"{field}.download_source"),
        install_comfyui_relative_path=safe_relative_path(
            data.get("install_comfyui_relative_path"),
            field_name=f"{field}.install_comfyui_relative_path",
        ),
    )


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _require_keys(data: dict[str, Any], allowed: set[str], field: str) -> None:
    unexpected = set(data) - allowed
    if unexpected:
        raise ValidationError(f"{field} contains unsupported fields")


def _list(payload: Any, field: str) -> list[Any]:
    if not isinstance(payload, list):
        raise ValidationError(f"{field} must be an array")
    return payload


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()


def _safe_identifier(value: Any, field: str) -> str:
    if not isinstance(value, str) or value == "":
        raise ValidationError(f"{field} must be a non-empty string")
    identifier = value
    if len(identifier) > MAX_IDENTIFIER_LENGTH or SAFE_IDENTIFIER_PATTERN.fullmatch(identifier) is None:
        raise ValidationError(f"{field} must be a safe identifier")
    return identifier


def _display_name(value: Any, field: str) -> str:
    name = _non_empty_string(value, field)
    if len(name) > MAX_DISPLAY_NAME_LENGTH or any(ord(character) < 32 or ord(character) == 127 for character in name):
        raise ValidationError(f"{field} must be a safe display name")
    return name
