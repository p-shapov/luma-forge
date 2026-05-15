from dataclasses import dataclass
from pathlib import Path
import re
from typing import Any

from app.errors import ValidationError
from auxiliary.paths import safe_custom_node_relative_path, safe_relative_path

MAX_IDENTIFIER_LENGTH = 128
MAX_DISPLAY_NAME_LENGTH = 256
SAFE_IDENTIFIER_PATTERN = re.compile(r"[A-Za-z0-9._-]+")


@dataclass(frozen=True)
class GitSource:
    repository_url: str
    revision: str


@dataclass(frozen=True)
class HuggingFaceSource:
    repository_id: str
    file_path: str
    revision: str


@dataclass(frozen=True)
class ModelAssetInstall:
    comfyui_relative_path: Path


@dataclass(frozen=True)
class ModelAsset:
    id: str
    name: str
    download_source: HuggingFaceSource
    install: ModelAssetInstall


@dataclass(frozen=True)
class CustomNodeInstall:
    comfyui_custom_nodes_relative_path: Path
    python_requirements_path: Path | None


@dataclass(frozen=True)
class CustomNode:
    id: str
    name: str
    git_source: GitSource
    install: CustomNodeInstall


@dataclass(frozen=True)
class WorkflowPreset:
    id: str
    version: str
    name: str
    required_comfyui_source: GitSource
    required_model_assets: list[ModelAsset]
    required_custom_nodes: list[CustomNode]


@dataclass(frozen=True)
class StartRequest:
    job_id: str
    workflow_preset: WorkflowPreset


@dataclass(frozen=True)
class CancelRequest:
    job_id: str


def parse_start_request(payload: Any) -> StartRequest:
    data = _object(payload, "request")
    job_id = _non_empty_string(data.get("job_id"), "job_id")
    workflow_preset = _parse_workflow_preset(data.get("workflow_preset"))
    return StartRequest(
        job_id=job_id,
        workflow_preset=workflow_preset,
    )


def parse_cancel_request(payload: Any) -> CancelRequest:
    data = _object(payload, "request")
    return CancelRequest(job_id=_non_empty_string(data.get("job_id"), "job_id"))


def _parse_workflow_preset(payload: Any) -> WorkflowPreset:
    data = _object(payload, "workflow_preset")
    return WorkflowPreset(
        id=_safe_identifier(data.get("id"), "workflow_preset.id"),
        version=_non_empty_string(data.get("version"), "workflow_preset.version"),
        name=_display_name(data.get("name"), "workflow_preset.name"),
        required_comfyui_source=_parse_git_source(
            data.get("required_comfyui_source"),
            "workflow_preset.required_comfyui_source",
        ),
        required_model_assets=[
            _parse_model_asset(item, f"workflow_preset.required_model_assets[{index}]")
            for index, item in enumerate(_list(data.get("required_model_assets"), "workflow_preset.required_model_assets"))
        ],
        required_custom_nodes=[
            _parse_custom_node(item, f"workflow_preset.required_custom_nodes[{index}]")
            for index, item in enumerate(_list(data.get("required_custom_nodes"), "workflow_preset.required_custom_nodes"))
        ],
    )


def _parse_git_source(payload: Any, field: str) -> GitSource:
    data = _object(payload, field)
    source_type = _non_empty_string(data.get("source_type"), f"{field}.source_type")
    if source_type != "git":
        raise ValidationError(f"{field}.source_type must be git")
    revision = _non_empty_string(data.get("revision"), f"{field}.revision")
    if not _is_immutable_git_revision(revision):
        raise ValidationError(f"{field}.revision must be a full immutable commit hash")
    return GitSource(
        repository_url=_non_empty_string(data.get("repository_url"), f"{field}.repository_url"),
        revision=revision,
    )


def _parse_huggingface_source(payload: Any, field: str) -> HuggingFaceSource:
    data = _object(payload, field)
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
    install = _object(data.get("install"), f"{field}.install")
    return ModelAsset(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        name=_display_name(data.get("name"), f"{field}.name"),
        download_source=_parse_huggingface_source(data.get("download_source"), f"{field}.download_source"),
        install=ModelAssetInstall(
            comfyui_relative_path=safe_relative_path(
                install.get("comfyui_relative_path"),
                field_name=f"{field}.install.comfyui_relative_path",
            ),
        ),
    )


def _parse_custom_node(payload: Any, field: str) -> CustomNode:
    data = _object(payload, field)
    install = _object(data.get("install"), f"{field}.install")
    requirements = install.get("python_requirements_path")
    return CustomNode(
        id=_safe_identifier(data.get("id"), f"{field}.id"),
        name=_display_name(data.get("name"), f"{field}.name"),
        git_source=_parse_git_source(data.get("git_source"), f"{field}.git_source"),
        install=CustomNodeInstall(
            comfyui_custom_nodes_relative_path=safe_custom_node_relative_path(
                install.get("comfyui_custom_nodes_relative_path"),
                field_name=f"{field}.install.comfyui_custom_nodes_relative_path",
            ),
            python_requirements_path=None
            if requirements is None
            else safe_relative_path(
                requirements,
                field_name=f"{field}.install.python_requirements_path",
            ),
        ),
    )


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


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


def _is_immutable_git_revision(value: str) -> bool:
    return re.fullmatch(r"[0-9a-f]{40}", value) is not None
