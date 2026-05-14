from pathlib import Path

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import PreparedEnvironmentError, ValidationError


def validate_prepared_environment(config: EndpointConfig) -> None:
    comfyui_root = config.comfyui_root
    if not comfyui_root.is_dir():
        raise PreparedEnvironmentError("Prepared ComfyUI directory is missing.")
    if not (comfyui_root / "main.py").is_file():
        raise PreparedEnvironmentError("Prepared ComfyUI entrypoint is missing.")

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
