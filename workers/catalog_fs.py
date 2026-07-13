from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any


ENTRY_ROOTS = {
    "execution_schema": "execution_schemas",
    "runtime_contract": "runtime_contracts",
    "runtime_preset": "runtime_presets",
    "workflow": "workflows",
}

WORKFLOW_FILES = (
    "metadata",
    "model_assets",
    "contract_requirements",
    "execution_contract",
    "workflow",
)


class ReleaseToolError(Exception):
    pass


def entry_file(
    catalog_root: Path,
    kind: str,
    entry_id: str,
    revision: str,
    filename: str,
) -> Path:
    if kind not in ENTRY_ROOTS:
        raise ReleaseToolError(f"unsupported catalog entry kind: {kind}")
    if not is_safe_identifier(entry_id):
        raise ReleaseToolError(f"invalid {kind.replace('_', ' ')} id")
    parse_semver(revision)
    path = catalog_root / "entries" / ENTRY_ROOTS[kind] / entry_id / revision / filename
    validate_catalog_path(catalog_root, path)
    if not path.is_file():
        raise ReleaseToolError(f"catalog entry file does not exist: {path}")
    return path


def catalog_ref(value: dict[str, Any], key: str, contract: str) -> tuple[str, str]:
    reference = dict_value(value, key)
    if string_value(reference, "contract") != contract:
        raise ReleaseToolError(f"{key} uses an unexpected contract")
    entry_id = string_value(reference, "id")
    revision = string_value(reference, "revision")
    if not is_safe_identifier(entry_id):
        raise ReleaseToolError(f"invalid {key} id")
    parse_semver(revision)
    return entry_id, revision


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ReleaseToolError(f"{path} must contain a JSON object")
    return value


def write_json(path: Path, value: dict[str, Any]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=False)
        handle.write("\n")


def validate_catalog_path(catalog_root: Path, path: Path) -> None:
    try:
        relative = path.relative_to(catalog_root)
    except ValueError as error:
        raise ReleaseToolError(f"catalog path escapes catalog root: {path}") from error
    if ".." in relative.parts:
        raise ReleaseToolError(f"catalog path escapes catalog root: {path}")

    current = catalog_root
    if current.is_symlink():
        raise ReleaseToolError(f"catalog path component must not be a symlink: {current}")
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            raise ReleaseToolError(
                f"catalog path component must not be a symlink: {current}"
            )


def validate_workflow_revision(catalog_root: Path, path: Path) -> None:
    validate_catalog_path(catalog_root, path)
    if path.is_symlink():
        raise ReleaseToolError(
            f"workflow revision directory must not be a symlink: {path}"
        )
    if not path.is_dir():
        raise ReleaseToolError(f"workflow revision directory does not exist: {path}")
    for name in WORKFLOW_FILES:
        document = path / name
        validate_catalog_path(catalog_root, document)
        if document.is_symlink():
            raise ReleaseToolError(
                f"workflow revision file must not be a symlink: {document}"
            )
        if not document.is_file():
            raise ReleaseToolError(f"workflow revision file does not exist: {document}")


def ensure_destination_available(catalog_root: Path, path: Path) -> None:
    validate_catalog_path(catalog_root, path.parent)
    if path.exists() or path.is_symlink():
        raise ReleaseToolError(f"catalog promotion destination already exists: {path}")


def dict_value(value: dict[str, Any], key: str) -> dict[str, Any]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise ReleaseToolError(f"{key} must be an object")
    return item


def list_value(value: dict[str, Any], key: str) -> list[Any]:
    item = value.get(key)
    if not isinstance(item, list):
        raise ReleaseToolError(f"{key} must be a list")
    return item


def string_value(value: dict[str, Any], key: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or item.strip() == "":
        raise ReleaseToolError(f"{key} must be a non-empty string")
    return item


def string_list_value(value: dict[str, Any], key: str) -> list[str]:
    item = value.get(key)
    if not isinstance(item, list) or any(not isinstance(entry, str) or entry.strip() == "" for entry in item):
        raise ReleaseToolError(f"{key} must be a non-empty string list")
    if not item:
        raise ReleaseToolError(f"{key} must not be empty")
    return item


def is_safe_identifier(value: str) -> bool:
    return re.fullmatch(r"[a-z][a-z0-9-]*", value) is not None


def parse_semver(value: str) -> tuple[int, int, int]:
    if not isinstance(value, str) or re.fullmatch(
        r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", value
    ) is None:
        raise ReleaseToolError("invalid contract version")
    major, minor, patch = value.split(".")
    return int(major), int(minor), int(patch)


def format_semver(value: tuple[int, int, int]) -> str:
    return f"{value[0]}.{value[1]}.{value[2]}"


def _revision_directories(catalog_root: Path, entries_root: Path) -> list[Path]:
    validate_catalog_path(catalog_root, entries_root)
    if not entries_root.is_dir():
        return []
    revisions = []
    for path in entries_root.iterdir():
        validate_catalog_path(catalog_root, path)
        if path.is_dir():
            revisions.append(path)
    return revisions


def latest_revision(catalog_root: Path, entries_root: Path) -> tuple[str, Path]:
    revisions = [
        (parse_semver(path.name), path)
        for path in _revision_directories(catalog_root, entries_root)
    ]
    if not revisions:
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    _version, path = max(revisions, key=lambda item: item[0])
    return path.name, path


def next_revision(
    catalog_root: Path, entries_root: Path, *, initial: str | None = None
) -> str:
    revisions = _revision_directories(catalog_root, entries_root)
    if not revisions:
        if initial is not None:
            parse_semver(initial)
            return initial
        raise ReleaseToolError(f"catalog entry has no revisions: {entries_root}")
    revision, _path = max(
        ((path.name, path) for path in revisions),
        key=lambda item: parse_semver(item[0]),
    )
    major, minor, patch = parse_semver(revision)
    return format_semver((major, minor, patch + 1))


def validate_image_ref(value: str) -> None:
    if re.fullmatch(r"[^:@\s]+(?:/[^:@\s]+)*@sha256:[0-9a-f]{64}", value) is None:
        raise ReleaseToolError(f"worker image ref must be digest-pinned: {value}")


def runpod_contract_requirements(value: dict[str, Any]) -> dict[str, Any]:
    for requirements in list_value(value, "contract_requirements"):
        if not isinstance(requirements, dict):
            raise ReleaseToolError("workflow catalog contains malformed contract requirements")
        if requirements.get("runtime_type") == "runpod":
            return requirements
    raise ReleaseToolError("workflow revision does not contain RunPod contract requirements")
