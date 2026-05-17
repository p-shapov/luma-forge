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
    image_runtime_root: Path
    workspace_root: Path
    python_overlay_path: Path
    custom_node_revisions: list[dict[str, str]]
    overlay_dependency_record_paths: list[Path]
    model_asset_paths: list[Path]
    prepared_at: str


def validate_prepared_environment(config: EndpointConfig) -> PreparedRuntimeManifest:
    manifest = load_runtime_manifest(config)
    _validate_runtime_manifest(config, manifest)

    comfyui_root = manifest.comfyui_root
    if not comfyui_root.is_dir():
        raise PreparedEnvironmentError("Image ComfyUI directory is missing.")
    if not (comfyui_root / "main.py").is_file():
        raise PreparedEnvironmentError("Image ComfyUI entrypoint is missing.")
    if not manifest.python_path.is_file():
        raise PreparedEnvironmentError("Image Python interpreter is missing.")
    if not manifest.python_overlay_path.is_dir():
        raise PreparedEnvironmentError("Prepared Python overlay directory is missing.")

    workflow_path = safe_child_path(manifest.workspace_root, config.workflow_relative_path, "workflow_relative_path")
    if not workflow_path.is_file():
        raise PreparedEnvironmentError("Prepared workflow definition is missing.")

    for path in config.required_model_paths:
        target = safe_child_path(manifest.workspace_root, path, "required_model_paths")
        if not target.is_file():
            raise PreparedEnvironmentError("Required model file is missing.")

    for path in config.required_custom_node_paths:
        target = safe_child_path(manifest.workspace_root, path, "required_custom_node_paths")
        if not target.exists():
            raise PreparedEnvironmentError("Required Custom Node path is missing.")

    for path in manifest.model_asset_paths:
        if not _is_under(path, manifest.workspace_root):
            raise PreparedRuntimeError("Prepared runtime model asset path is outside the workspace.")
        if not path.is_file():
            raise PreparedEnvironmentError("Prepared model asset is missing.")

    for path in manifest.overlay_dependency_record_paths:
        if not _is_under(path, manifest.workspace_root):
            raise PreparedRuntimeError("Prepared runtime overlay dependency record path is outside the workspace.")
        if not path.is_file():
            raise PreparedEnvironmentError("Prepared overlay dependency record is missing.")

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
            image_runtime_root=Path(_required_string(payload, "image_runtime_root")),
            workspace_root=Path(_required_string(payload, "workspace_root")),
            python_overlay_path=Path(_required_string(payload, "python_overlay_path")),
            custom_node_revisions=_custom_node_revision_payload(payload),
            overlay_dependency_record_paths=[
                Path(path) for path in _string_list(payload, "overlay_dependency_record_paths")
            ],
            model_asset_paths=[Path(path) for path in _string_list(payload, "model_asset_paths")],
            prepared_at=_required_string(payload, "prepared_at"),
        )
    except (TypeError, ValueError) as error:
        raise PreparedRuntimeError("Prepared runtime manifest is invalid.") from error


def workflow_path(config: EndpointConfig, runtime: PreparedRuntimeManifest) -> Path:
    return safe_child_path(runtime.workspace_root, config.workflow_relative_path, "workflow_relative_path")


def safe_child_path(root: Path, relative_path: Path, field_name: str) -> Path:
    if relative_path.is_absolute() or not relative_path.parts:
        raise ValidationError(f"{field_name} must be a relative path")
    if any(part in ("", ".", "..") for part in relative_path.parts):
        raise ValidationError(f"{field_name} contains an unsafe path segment")

    root_resolved = root.resolve(strict=False)
    target = (root_resolved / relative_path).resolve(strict=False)
    if target != root_resolved and root_resolved not in target.parents:
        raise ValidationError(f"{field_name} must resolve under root")
    return target


def _validate_runtime_manifest(config: EndpointConfig, manifest: PreparedRuntimeManifest) -> None:
    if manifest.environment_kind != ENVIRONMENT_KIND:
        raise PreparedRuntimeError("Prepared runtime environment kind is invalid.")
    if manifest.image_runtime_root.resolve(strict=False) != config.image_runtime_root_path.resolve(strict=False):
        raise PreparedRuntimeError("Prepared runtime image root path is invalid.")
    if manifest.comfyui_root.resolve(strict=False) != config.comfyui_root.resolve(strict=False):
        raise PreparedRuntimeError("Prepared runtime image ComfyUI path is invalid.")
    if manifest.python_path.resolve(strict=False) != config.image_python_path.resolve(strict=False):
        raise PreparedRuntimeError("Prepared runtime image Python path is invalid.")
    if manifest.workspace_root.resolve(strict=False) != config.workspace_mount_path.resolve(strict=False):
        raise PreparedRuntimeError("Prepared runtime workspace path is invalid.")
    if not _is_under(manifest.python_overlay_path, config.workspace_mount_path):
        raise PreparedRuntimeError("Prepared runtime overlay path is outside the workspace.")

def _is_under(path: Path, root: Path) -> bool:
    root_resolved = root.resolve(strict=False)
    resolved = path.resolve(strict=False)
    return resolved == root_resolved or root_resolved in resolved.parents


def _custom_node_revision_payload(payload: dict[str, object]) -> list[dict[str, str]]:
    value = payload.get("custom_node_revisions")
    if not isinstance(value, list):
        raise ValueError("custom_node_revisions is required")
    revisions = []
    for entry in value:
        if not isinstance(entry, dict):
            raise ValueError("custom_node_revisions entry must be an object")
        revisions.append(
            {
                "id": _required_string(entry, "id"),
                "revision": _required_string(entry, "revision"),
            }
        )
    return revisions


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
