from dataclasses import dataclass
import json
from pathlib import Path

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import PreparedEnvironmentError, PreparedRuntimeError, ValidationError


ENVIRONMENT_KIND = "image_baked_comfyui_runtime"


@dataclass(frozen=True)
class PreparedRuntimeManifest:
    environment_kind: str
    python_path: Path
    comfyui_root: Path
    python_version: str
    platform: str
    comfyui_revision: str
    runtime_contract_id: str
    runtime_contract_version: str
    implementation_revision: str
    provisioner_image_ref: str
    endpoint_image_ref: str
    base_dependency_record_paths: list[Path]


def validate_prepared_environment(config: EndpointConfig) -> PreparedRuntimeManifest:
    manifest = load_runtime_manifest(config)
    _validate_runtime_manifest(config, manifest)

    comfyui_root = config.comfyui_root
    if not comfyui_root.is_dir():
        raise PreparedEnvironmentError("Prepared ComfyUI directory is missing.")
    if not (comfyui_root / "main.py").is_file():
        raise PreparedEnvironmentError("Prepared ComfyUI entrypoint is missing.")
    if not manifest.python_path.is_file():
        raise PreparedEnvironmentError("Prepared Python interpreter is missing.")

    workflow_path = safe_child_path(comfyui_root, config.workflow_relative_path, "workflow_relative_path")
    if not workflow_path.is_file():
        raise PreparedEnvironmentError("Prepared workflow definition is missing.")

    for path in config.required_model_paths:
        target = safe_child_path(comfyui_root, path, "required_model_paths")
        if not target.is_file():
            raise PreparedEnvironmentError("Required model file is missing.")

    for path in config.required_custom_node_paths:
        target = safe_child_path(comfyui_root, path, "required_custom_node_paths")
        if not target.exists():
            raise PreparedEnvironmentError("Required Custom Node path is missing.")

    return manifest


def load_runtime_manifest(config: EndpointConfig) -> PreparedRuntimeManifest:
    try:
        payload = json.loads(config.runtime_manifest_path.read_text(encoding="utf-8"))
    except OSError as error:
        raise PreparedRuntimeError("Prepared runtime manifest is missing.") from error
    except json.JSONDecodeError as error:
        raise PreparedRuntimeError("Prepared runtime manifest is invalid.") from error

    try:
        if not isinstance(payload, dict):
            raise ValueError("runtime manifest must be an object")
        return PreparedRuntimeManifest(
            environment_kind=_required_string(payload, "environment_kind"),
            python_path=Path(_required_string(payload, "python_path")),
            comfyui_root=Path(_required_string(payload, "comfyui_root")),
            python_version=_required_string(payload, "python_version"),
            platform=_required_string(payload, "platform"),
            comfyui_revision=_required_string(payload, "comfyui_revision"),
            runtime_contract_id=_required_string(payload, "runtime_contract_id"),
            runtime_contract_version=_required_string(payload, "runtime_contract_version"),
            implementation_revision=_required_string(payload, "implementation_revision"),
            provisioner_image_ref=_required_string(payload, "provisioner_image_ref"),
            endpoint_image_ref=_required_string(payload, "endpoint_image_ref"),
            base_dependency_record_paths=[
                Path(path) for path in _string_list(payload, "base_dependency_record_paths")
            ],
        )
    except (TypeError, ValueError) as error:
        raise PreparedRuntimeError("Prepared runtime manifest is invalid.") from error


def workflow_path(config: EndpointConfig) -> Path:
    return safe_child_path(config.comfyui_root, config.workflow_relative_path, "workflow_relative_path")


def safe_child_path(root: Path, relative_path: Path, field_name: str) -> Path:
    if relative_path.is_absolute() or not relative_path.parts:
        raise ValidationError(f"{field_name} must be a relative path")
    if any(part in ("", ".", "..") for part in relative_path.parts):
        raise ValidationError(f"{field_name} contains an unsafe path segment")

    root_resolved = root.resolve(strict=False)
    target = (root_resolved / relative_path).resolve(strict=False)
    if target != root_resolved and root_resolved not in target.parents:
        raise ValidationError(f"{field_name} must resolve under ComfyUI root")
    return target


def _validate_runtime_manifest(config: EndpointConfig, manifest: PreparedRuntimeManifest) -> None:
    if manifest.environment_kind != ENVIRONMENT_KIND:
        raise PreparedRuntimeError("Prepared runtime environment kind is invalid.")
    if manifest.comfyui_root.resolve(strict=False) != config.comfyui_root.resolve(strict=False):
        raise PreparedRuntimeError("Prepared runtime ComfyUI path is invalid.")
    workspace = config.workspace_mount_path.resolve(strict=False)
    for field_name, path in (
        ("python_path", manifest.python_path),
        *[
            (f"base_dependency_record_paths[{index}]", path)
            for index, path in enumerate(manifest.base_dependency_record_paths)
        ],
    ):
        resolved = path.resolve(strict=False)
        if resolved != workspace and workspace not in resolved.parents:
            raise PreparedRuntimeError(f"Prepared runtime {field_name} is outside the workspace.")


def _required_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or value.strip() == "":
        raise ValueError(f"{key} is required")
    return value


def _string_list(payload: dict[str, object], key: str) -> list[str]:
    value = payload.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) and item.strip() for item in value):
        raise ValueError(f"{key} is required")
    return value
