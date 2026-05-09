from pathlib import Path, PurePosixPath

from provisioner_worker.errors import ValidationError


def safe_relative_path(value: object, *, field_name: str) -> Path:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field_name} must be a non-empty relative path")

    raw = value.strip().replace("\\", "/")
    path = PurePosixPath(raw)
    if path.is_absolute():
        raise ValidationError(f"{field_name} must be relative")
    if any(part in ("", ".", "..") for part in path.parts):
        raise ValidationError(f"{field_name} must not contain empty, current, or parent segments")

    return Path(*path.parts)


def safe_child_path(root: Path, relative_value: object, *, field_name: str) -> Path:
    relative_path = safe_relative_path(relative_value, field_name=field_name)
    root_resolved = root.resolve(strict=False)
    target = (root_resolved / relative_path).resolve(strict=False)

    if target != root_resolved and root_resolved not in target.parents:
        raise ValidationError(f"{field_name} must resolve under {root_resolved}")

    return target

