from pathlib import Path, PurePosixPath

from app.errors import PathValidationError


def safe_relative_path(value: object, *, field_name: str) -> Path:
    if not isinstance(value, str) or value.strip() == "":
        raise PathValidationError(f"{field_name} must be a non-empty relative path")

    raw = value.strip().replace("\\", "/")
    path = PurePosixPath(raw)
    if path.is_absolute():
        raise PathValidationError(f"{field_name} must be relative")
    if any(part in ("", ".", "..") for part in path.parts):
        raise PathValidationError(f"{field_name} must not contain empty, current, or parent segments")

    return Path(*path.parts)


def safe_custom_node_relative_path(value: object, *, field_name: str) -> Path:
    path = safe_relative_path(value, field_name=field_name)
    parts = path.parts
    if len(parts) < 2 or parts[0] != "custom_nodes":
        raise PathValidationError(f"{field_name} must resolve under custom_nodes")
    return path


def safe_child_path(root: Path, relative_value: object, *, field_name: str) -> Path:
    relative_path = safe_relative_path(relative_value, field_name=field_name)
    root_resolved = root.resolve(strict=False)
    target = (root_resolved / relative_path).resolve(strict=False)

    if target != root_resolved and root_resolved not in target.parents:
        raise PathValidationError(f"{field_name} must resolve under {root_resolved}")

    return target


def safe_custom_node_child_path(root: Path, relative_value: object, *, field_name: str) -> Path:
    relative_path = safe_custom_node_relative_path(relative_value, field_name=field_name)
    root_resolved = root.resolve(strict=False)
    custom_nodes_root = (root_resolved / "custom_nodes").resolve(strict=False)
    target = (root_resolved / relative_path).resolve(strict=False)

    if custom_nodes_root not in target.parents:
        raise PathValidationError(f"{field_name} must resolve under {custom_nodes_root}")

    return target
