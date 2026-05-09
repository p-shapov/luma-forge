from dataclasses import dataclass
from pathlib import Path
from typing import Any

from provisioner_worker.errors import ValidationError
from provisioner_worker.paths import safe_relative_path


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
    file_size_bytes: int
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
    workspace_mount_path: Path
    workflow_preset: WorkflowPreset


@dataclass(frozen=True)
class CancelRequest:
    job_id: str


def parse_start_request(payload: Any) -> StartRequest:
    data = _object(payload, "request")
    job_id = _non_empty_string(data.get("job_id"), "job_id")
    workspace_mount_path = Path(_non_empty_string(data.get("workspace_mount_path"), "workspace_mount_path"))
    workflow_preset = _parse_workflow_preset(data.get("workflow_preset"))
    return StartRequest(
        job_id=job_id,
        workspace_mount_path=workspace_mount_path,
        workflow_preset=workflow_preset,
    )


def parse_cancel_request(payload: Any) -> CancelRequest:
    data = _object(payload, "request")
    return CancelRequest(job_id=_non_empty_string(data.get("job_id"), "job_id"))


def _parse_workflow_preset(payload: Any) -> WorkflowPreset:
    data = _object(payload, "workflow_preset")
    return WorkflowPreset(
        id=_non_empty_string(data.get("id"), "workflow_preset.id"),
        version=_non_empty_string(data.get("version"), "workflow_preset.version"),
        name=_non_empty_string(data.get("name"), "workflow_preset.name"),
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
    return GitSource(
        repository_url=_non_empty_string(data.get("repository_url"), f"{field}.repository_url"),
        revision=_non_empty_string(data.get("revision"), f"{field}.revision"),
    )


def _parse_huggingface_source(payload: Any, field: str) -> HuggingFaceSource:
    data = _object(payload, field)
    source_type = _non_empty_string(data.get("source_type"), f"{field}.source_type")
    if source_type != "huggingface":
        raise ValidationError(f"{field}.source_type must be huggingface")
    return HuggingFaceSource(
        repository_id=_non_empty_string(data.get("repository_id"), f"{field}.repository_id"),
        file_path=_non_empty_string(data.get("file_path"), f"{field}.file_path"),
        revision=_non_empty_string(data.get("revision"), f"{field}.revision"),
    )


def _parse_model_asset(payload: Any, field: str) -> ModelAsset:
    data = _object(payload, field)
    install = _object(data.get("install"), f"{field}.install")
    file_size_bytes = data.get("file_size_bytes")
    if not isinstance(file_size_bytes, int) or file_size_bytes < 0:
        raise ValidationError(f"{field}.file_size_bytes must be a non-negative integer")
    return ModelAsset(
        id=_non_empty_string(data.get("id"), f"{field}.id"),
        name=_non_empty_string(data.get("name"), f"{field}.name"),
        file_size_bytes=file_size_bytes,
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
        id=_non_empty_string(data.get("id"), f"{field}.id"),
        name=_non_empty_string(data.get("name"), f"{field}.name"),
        git_source=_parse_git_source(data.get("git_source"), f"{field}.git_source"),
        install=CustomNodeInstall(
            comfyui_custom_nodes_relative_path=safe_relative_path(
                install.get("comfyui_custom_nodes_relative_path"),
                field_name=f"{field}.install.comfyui_custom_nodes_relative_path",
            ),
            python_requirements_path=None
            if requirements is None or requirements == ""
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

