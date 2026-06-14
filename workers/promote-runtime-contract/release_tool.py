#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


class ReleaseToolError(Exception):
    pass


def load_contract(path: Path) -> dict[str, Any]:
    contract = _load_yaml(path)
    schema = _load_json(Path(__file__).resolve().parent / "schema.json")
    _validate_with_schema(contract, schema)
    _validate_contract(contract, path)
    return contract


def find_contract(catalog: dict[str, Any], contract_id: str) -> dict[str, Any] | None:
    for contract in _list_value(catalog, "contracts"):
        if not isinstance(contract, dict):
            raise ReleaseToolError("runtime contracts contains a malformed contract entry")
        if contract.get("id") == contract_id:
            return contract
    return None


def find_revision(contract: dict[str, Any], contract_version: str) -> dict[str, Any] | None:
    for revision in _list_value(contract, "revisions"):
        if not isinstance(revision, dict):
            raise ReleaseToolError("runtime contracts contains a malformed revision entry")
        if revision.get("version") == contract_version:
            return revision
    return None


def next_contract_version(*, contract: dict[str, Any], catalog: dict[str, Any]) -> str:
    declared_version = _parse_semver(contract["contract"]["version"])
    catalog_contract = find_contract(catalog, contract["contract"]["id"])
    if catalog_contract is None:
        return _format_semver(declared_version)

    revisions = _list_value(catalog_contract, "revisions")
    if not revisions:
        return _format_semver(declared_version)

    latest = max(_parse_semver(_string_value(revision, "version")) for revision in revisions)
    next_patch = (latest[0], latest[1], latest[2] + 1)
    return _format_semver(max(declared_version, next_patch))


def validate_catalog_compatibility(*, contract: dict[str, Any], catalog: dict[str, Any]) -> None:
    contract_id = contract["contract"]["id"]
    contract_version = contract["contract"]["version"]
    catalog_contract = find_contract(catalog, contract_id)
    if catalog_contract is None:
        return
    _string_value(catalog_contract, "id")
    revision = find_revision(catalog_contract, contract_version)
    if revision is None:
        return
    _string_value(revision, "version")
    _validate_image_ref(_string_value(revision, "image_ref"))


def promote_runtime_image(
    *,
    contract: dict[str, Any],
    catalog: dict[str, Any],
    image_ref: str,
    contract_version: str | None = None,
) -> dict[str, Any]:
    validate_catalog_compatibility(contract=contract, catalog=catalog)
    _validate_image_ref(image_ref)
    contracts = _list_value(catalog, "contracts")
    contract_id = contract["contract"]["id"]
    resolved_contract_version = contract_version or next_contract_version(contract=contract, catalog=catalog)
    _parse_semver(resolved_contract_version)
    catalog_contract = find_contract(catalog, contract_id)
    if catalog_contract is None:
        contracts.append(
            {
                "id": contract_id,
                "revisions": [
                    {
                        "version": resolved_contract_version,
                        "image_ref": image_ref,
                    }
                ],
            }
        )
    else:
        revisions = _list_value(catalog_contract, "revisions")
        revision = find_revision(catalog_contract, resolved_contract_version)
        if revision is not None:
            raise ReleaseToolError(f"runtime contracts revision already exists: {contract_id} {resolved_contract_version}")
        revisions.append(
            {
                "version": resolved_contract_version,
                "image_ref": image_ref,
            }
        )
    return catalog


def update_runtime_workflow_catalog(
    *,
    catalog: dict[str, Any],
    contract_id: str,
    contract_version: str,
) -> dict[str, Any]:
    updated = False
    workflow_presets = _list_value(catalog, "workflow_presets")
    for preset in workflow_presets:
        if not isinstance(preset, dict):
            raise ReleaseToolError("workflow catalog contains a malformed preset entry")
        for runtime_requirements in _runpod_runtime_requirements(preset):
            endpoint_contract = _dict_value(runtime_requirements, "endpoint_contract")
            if endpoint_contract.get("id") == contract_id:
                endpoint_contract["version"] = contract_version
                updated = True
    if not updated:
        raise ReleaseToolError(f"workflow catalog does not reference endpoint contract: {contract_id}")
    return catalog


def contract_outputs(
    contract: dict[str, Any],
    contract_path: Path,
    catalog: dict[str, Any] | None = None,
) -> dict[str, str]:
    runtime = contract["runtime"]
    packages_json = json.dumps(runtime["pytorch"]["packages"], separators=(",", ":"))
    contract_version = (
        next_contract_version(contract=contract, catalog=catalog)
        if catalog is not None
        else contract["contract"]["version"]
    )
    return {
        "contract": str(contract_path),
        "contract_id": contract["contract"]["id"],
        "contract_version": contract_version,
        "runtime_python_version": runtime["python_version"],
        "comfyui_revision": runtime["comfyui_revision"],
        "pytorch_index_url": runtime["pytorch"]["index_url"],
        "pytorch_packages_json": packages_json,
        "bundled_workflow_path": str(resolve_bundled_workflow_path(contract, contract_path)),
    }


def resolve_bundled_workflow_path(contract: dict[str, Any], contract_path: Path) -> Path:
    repository_root = Path(__file__).resolve().parents[2]
    workflow_path = repository_root / "bundled" / "workflows" / "comfyui-hidream-o1-dev.json"
    if not workflow_path.is_file():
        raise ReleaseToolError(f"bundled workflow file does not exist: {workflow_path}")
    return workflow_path.relative_to(repository_root)


def write_github_outputs(outputs: dict[str, str], path: Path) -> None:
    with path.open("a", encoding="utf-8") as handle:
        for key, value in outputs.items():
            if "\n" in value or "\r" in value:
                raise ReleaseToolError(f"output contains newline: {key}")
            handle.write(f"{key}={value}\n")


def _load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ReleaseToolError(f"{path} must contain a JSON object")
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, sort_keys=False)
        handle.write("\n")


def _runpod_runtime_requirements(preset: dict[str, Any]) -> list[dict[str, Any]]:
    requirements: list[dict[str, Any]] = []
    for revision in _list_value(preset, "revisions"):
        if not isinstance(revision, dict):
            raise ReleaseToolError("workflow catalog contains a malformed revision entry")
        requirements.append(_dict_value(revision, "runpod_runtime_requirements"))
    return requirements


def _load_yaml(path: Path) -> dict[str, Any]:
    try:
        import yaml  # type: ignore
    except ModuleNotFoundError:
        value = _load_simple_yaml(path)
    else:
        with path.open("r", encoding="utf-8") as handle:
            value = yaml.safe_load(handle)
    if not isinstance(value, dict):
        raise ReleaseToolError(f"{path} must contain a YAML object")
    return value


def _validate_with_schema(value: Any, schema: dict[str, Any]) -> None:
    try:
        from jsonschema import Draft202012Validator, FormatChecker  # type: ignore
    except ModuleNotFoundError:
        _validate_schema_subset(value, schema, path="$")
    else:
        validator = Draft202012Validator(schema, format_checker=FormatChecker())
        errors = sorted(validator.iter_errors(value), key=lambda error: list(error.path))
        if errors:
            error = errors[0]
            location = "$" + "".join(f".{part}" for part in error.path)
            raise ReleaseToolError(f"contract schema validation failed at {location}: {error.message}")


def _validate_schema_subset(value: Any, schema: dict[str, Any], *, path: str) -> None:
    expected_type = schema.get("type")
    if expected_type == "object":
        if not isinstance(value, dict):
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected object")
        required = schema.get("required", [])
        for key in required:
            if key not in value:
                raise ReleaseToolError(f"contract schema validation failed at {path}.{key}: missing required property")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            extra = sorted(set(value) - set(properties))
            if extra:
                raise ReleaseToolError(f"contract schema validation failed at {path}.{extra[0]}: unsupported property")
        for key, nested_schema in properties.items():
            if key in value:
                _validate_schema_subset(value[key], nested_schema, path=f"{path}.{key}")
        return

    if expected_type == "array":
        if not isinstance(value, list):
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected array")
        min_items = schema.get("minItems")
        if isinstance(min_items, int) and len(value) < min_items:
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected at least {min_items} items")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                _validate_schema_subset(item, item_schema, path=f"{path}[{index}]")
        return

    if expected_type == "string":
        if not isinstance(value, str):
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected string")
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected at least {min_length} characters")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.fullmatch(pattern, value) is None:
            raise ReleaseToolError(f"contract schema validation failed at {path}: pattern mismatch")
        if schema.get("format") == "uri" and not _is_uri(value):
            raise ReleaseToolError(f"contract schema validation failed at {path}: expected uri")
        return

    raise ReleaseToolError(f"contract schema validation failed at {path}: unsupported schema")


def _is_uri(value: str) -> bool:
    parsed = urlparse(value)
    return bool(parsed.scheme and parsed.netloc)


def _load_simple_yaml(path: Path) -> dict[str, Any]:
    raw_lines = path.read_text(encoding="utf-8").splitlines()
    lines = []
    for line in raw_lines:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        indent = len(line) - len(line.lstrip(" "))
        lines.append((indent, line.strip()))
    value, index = _parse_yaml_block(lines, 0, 0)
    if index != len(lines) or not isinstance(value, dict):
        raise ReleaseToolError(f"{path} contains unsupported YAML")
    return value


def _parse_yaml_block(lines: list[tuple[int, str]], index: int, indent: int) -> tuple[Any, int]:
    if index >= len(lines):
        return {}, index
    if lines[index][0] < indent:
        return {}, index
    is_list = lines[index][0] == indent and lines[index][1].startswith("- ")
    if is_list:
        items = []
        while index < len(lines) and lines[index][0] == indent and lines[index][1].startswith("- "):
            item = lines[index][1][2:].strip()
            if item == "":
                nested, index = _parse_yaml_block(lines, index + 1, indent + 2)
                items.append(nested)
            else:
                items.append(_parse_scalar(item))
                index += 1
        return items, index

    mapping: dict[str, Any] = {}
    while index < len(lines):
        current_indent, text = lines[index]
        if current_indent < indent or current_indent != indent or text.startswith("- "):
            break
        key, separator, raw_value = text.partition(":")
        if separator != ":" or not key:
            raise ReleaseToolError(f"unsupported YAML line: {text}")
        raw_value = raw_value.strip()
        if raw_value:
            mapping[key] = _parse_scalar(raw_value)
            index += 1
        else:
            nested, index = _parse_yaml_block(lines, index + 1, indent + 2)
            mapping[key] = nested
    return mapping, index


def _parse_scalar(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("'", '"'):
        return value[1:-1]
    return value


def _validate_contract(value: dict[str, Any], contract_path: Path) -> None:
    contract = _dict_value(value, "contract")
    runtime = _dict_value(value, "runtime")

    contract_id = _string_value(contract, "id")
    contract_version = _string_value(contract, "version")
    if not _is_safe_identifier(contract_id):
        raise ReleaseToolError("invalid contract id")
    _parse_semver(contract_version)

    resolve_bundled_workflow_path(value, contract_path)

    _string_value(runtime, "python_version")
    comfyui_revision = _string_value(runtime, "comfyui_revision")
    if not re.fullmatch(r"[0-9a-f]{40}", comfyui_revision):
        raise ReleaseToolError("invalid ComfyUI revision")
    pytorch = _dict_value(runtime, "pytorch")
    _string_value(pytorch, "index_url")
    _string_list_value(pytorch, "packages")


def _dict_value(value: dict[str, Any], key: str) -> dict[str, Any]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise ReleaseToolError(f"{key} must be an object")
    return item


def _list_value(value: dict[str, Any], key: str) -> list[Any]:
    item = value.get(key)
    if not isinstance(item, list):
        raise ReleaseToolError(f"{key} must be a list")
    return item


def _string_value(value: dict[str, Any], key: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or item.strip() == "":
        raise ReleaseToolError(f"{key} must be a non-empty string")
    return item


def _string_list_value(value: dict[str, Any], key: str) -> list[str]:
    item = value.get(key)
    if not isinstance(item, list) or any(not isinstance(entry, str) or entry.strip() == "" for entry in item):
        raise ReleaseToolError(f"{key} must be a non-empty string list")
    if not item:
        raise ReleaseToolError(f"{key} must not be empty")
    return item


def _validate_image_ref(value: str) -> None:
    if re.fullmatch(r"[^:@\s]+(?:/[^:@\s]+)*@sha256:[0-9a-f]{64}", value) is None:
        raise ReleaseToolError(f"worker image ref must be digest-pinned: {value}")


def _is_safe_identifier(value: str) -> bool:
    return re.fullmatch(r"[a-z][a-z0-9-]*", value) is not None


def _parse_semver(value: str) -> tuple[int, int, int]:
    if not isinstance(value, str):
        raise ReleaseToolError("invalid contract version")
    parts = value.split(".")
    if len(parts) != 3:
        raise ReleaseToolError("invalid contract version")
    parsed = []
    for part in parts:
        if not part.isdigit() or (len(part) > 1 and part.startswith("0")):
            raise ReleaseToolError("invalid contract version")
        parsed.append(int(part))
    return (parsed[0], parsed[1], parsed[2])


def _format_semver(value: tuple[int, int, int]) -> str:
    return f"{value[0]}.{value[1]}.{value[2]}"


def _cmd_resolve(args: argparse.Namespace) -> None:
    contract_path = Path(args.contract)
    contract = load_contract(contract_path)
    catalog = _load_json(Path(args.catalog)) if args.catalog else None
    outputs = contract_outputs(contract, contract_path, catalog)
    if args.github_output:
        write_github_outputs(outputs, Path(args.github_output))
    else:
        for key, value in outputs.items():
            print(f"{key}={value}")


def _cmd_validate_catalog(args: argparse.Namespace) -> None:
    validate_catalog_compatibility(contract=load_contract(Path(args.contract)), catalog=_load_json(Path(args.catalog)))


def _cmd_promote_runtime_image(args: argparse.Namespace) -> None:
    contract = load_contract(Path(args.contract))
    catalog_path = Path(args.catalog)
    runtime_catalog = _load_json(catalog_path)
    contract_version = args.contract_version or next_contract_version(contract=contract, catalog=runtime_catalog)
    updated = promote_runtime_image(
        contract=contract,
        catalog=runtime_catalog,
        image_ref=args.image_ref,
        contract_version=contract_version,
    )
    updated_workflow_catalog = None
    if args.workflow_catalog:
        workflow_catalog_path = Path(args.workflow_catalog)
        updated_workflow_catalog = update_runtime_workflow_catalog(
            catalog=_load_json(workflow_catalog_path),
            contract_id=contract["contract"]["id"],
            contract_version=contract_version,
        )
    _write_json(catalog_path, updated)
    if args.workflow_catalog and updated_workflow_catalog is not None:
        _write_json(Path(args.workflow_catalog), updated_workflow_catalog)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Endpoint contract release and promotion helper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve = subparsers.add_parser("resolve", help="resolve contract outputs")
    resolve.add_argument("--contract", required=True)
    resolve.add_argument("--catalog")
    resolve.add_argument("--github-output")
    resolve.set_defaults(func=_cmd_resolve)

    validate_catalog = subparsers.add_parser("validate-catalog", help="validate catalog shape")
    validate_catalog.add_argument("--contract", required=True)
    validate_catalog.add_argument("--catalog", required=True)
    validate_catalog.set_defaults(func=_cmd_validate_catalog)

    promote_runtime = subparsers.add_parser(
        "promote-runtime-image",
        help="promote a digest-pinned endpoint image into Runtime Contracts",
    )
    promote_runtime.add_argument("--contract", required=True)
    promote_runtime.add_argument("--catalog", required=True)
    promote_runtime.add_argument("--image-ref", required=True)
    promote_runtime.add_argument("--contract-version")
    promote_runtime.add_argument("--workflow-catalog")
    promote_runtime.set_defaults(func=_cmd_promote_runtime_image)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        args.func(args)
    except ReleaseToolError as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
