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


def load_recipe(path: Path) -> dict[str, Any]:
    recipe = _load_yaml(path)
    schema = _load_json(path.parent / "schema.json")
    _validate_with_schema(recipe, schema)
    _validate_recipe(recipe)
    return recipe


def find_contract(catalog: dict[str, Any], contract_id: str) -> dict[str, Any] | None:
    for contract in _list_value(catalog, "contracts"):
        if not isinstance(contract, dict):
            raise ReleaseToolError("runtime catalog contains a malformed contract entry")
        if contract.get("id") == contract_id:
            return contract
    return None


def find_revision(contract: dict[str, Any], contract_version: str) -> dict[str, Any] | None:
    for revision in _list_value(contract, "revisions"):
        if not isinstance(revision, dict):
            raise ReleaseToolError("runtime catalog contains a malformed revision entry")
        if revision.get("version") == contract_version:
            return revision
    return None


def validate_catalog_compatibility(*, recipe: dict[str, Any], catalog: dict[str, Any]) -> None:
    contract_id = recipe["contract"]["id"]
    contract_version = recipe["contract"]["version"]
    contract = find_contract(catalog, contract_id)
    if contract is None:
        return
    _string_value(contract, "id")
    revision = find_revision(contract, contract_version)
    if revision is None:
        return
    _string_value(revision, "version")
    _validate_image_ref(_string_value(revision, "provisioner_image_ref"))
    _validate_image_ref(_string_value(revision, "endpoint_image_ref"))


def update_catalog(
    *,
    recipe: dict[str, Any],
    catalog: dict[str, Any],
    provisioner_ref: str,
    endpoint_ref: str,
) -> dict[str, Any]:
    validate_catalog_compatibility(recipe=recipe, catalog=catalog)
    _validate_image_ref(provisioner_ref)
    _validate_image_ref(endpoint_ref)
    contracts = _list_value(catalog, "contracts")
    contract_id = recipe["contract"]["id"]
    contract_version = recipe["contract"]["version"]
    contract = find_contract(catalog, contract_id)
    if contract is None:
        contracts.append(
            {
                "id": contract_id,
                "revisions": [
                    {
                        "version": contract_version,
                        "provisioner_image_ref": provisioner_ref,
                        "endpoint_image_ref": endpoint_ref,
                    }
                ],
            }
        )
    else:
        revisions = _list_value(contract, "revisions")
        revision = find_revision(contract, contract_version)
        if revision is None:
            revisions.append(
                {
                    "version": contract_version,
                    "provisioner_image_ref": provisioner_ref,
                    "endpoint_image_ref": endpoint_ref,
                }
            )
        else:
            revision["provisioner_image_ref"] = provisioner_ref
            revision["endpoint_image_ref"] = endpoint_ref
    return catalog


def recipe_outputs(recipe: dict[str, Any], recipe_path: Path) -> dict[str, str]:
    runtime = recipe["runtime"]
    packages_json = json.dumps(runtime["pytorch"]["packages"], separators=(",", ":"))
    requirements_json = json.dumps(runtime["base_requirements"], separators=(",", ":"))
    return {
        "recipe": str(recipe_path),
        "contract_id": recipe["contract"]["id"],
        "contract_version": recipe["contract"]["version"],
        "runtime_python_version": runtime["python_version"],
        "runtime_platform": runtime["platform"],
        "comfyui_repository": runtime["comfyui"]["repository_url"],
        "comfyui_revision": runtime["comfyui"]["revision"],
        "pytorch_index_url": runtime["pytorch"]["index_url"],
        "pytorch_packages_json": packages_json,
        "base_requirements_json": requirements_json,
    }


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
            raise ReleaseToolError(f"recipe schema validation failed at {location}: {error.message}")


def _validate_schema_subset(value: Any, schema: dict[str, Any], *, path: str) -> None:
    expected_type = schema.get("type")
    if expected_type == "object":
        if not isinstance(value, dict):
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected object")
        required = schema.get("required", [])
        for key in required:
            if key not in value:
                raise ReleaseToolError(f"recipe schema validation failed at {path}.{key}: missing required property")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            extra = sorted(set(value) - set(properties))
            if extra:
                raise ReleaseToolError(f"recipe schema validation failed at {path}.{extra[0]}: unsupported property")
        for key, nested_schema in properties.items():
            if key in value:
                _validate_schema_subset(value[key], nested_schema, path=f"{path}.{key}")
        return

    if expected_type == "array":
        if not isinstance(value, list):
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected array")
        min_items = schema.get("minItems")
        if isinstance(min_items, int) and len(value) < min_items:
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected at least {min_items} items")
        item_schema = schema.get("items")
        if isinstance(item_schema, dict):
            for index, item in enumerate(value):
                _validate_schema_subset(item, item_schema, path=f"{path}[{index}]")
        return

    if expected_type == "string":
        if not isinstance(value, str):
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected string")
        min_length = schema.get("minLength")
        if isinstance(min_length, int) and len(value) < min_length:
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected at least {min_length} characters")
        pattern = schema.get("pattern")
        if isinstance(pattern, str) and re.fullmatch(pattern, value) is None:
            raise ReleaseToolError(f"recipe schema validation failed at {path}: pattern mismatch")
        if schema.get("format") == "uri" and not _is_uri(value):
            raise ReleaseToolError(f"recipe schema validation failed at {path}: expected uri")
        return

    raise ReleaseToolError(f"recipe schema validation failed at {path}: unsupported schema")


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


def _validate_recipe(recipe: dict[str, Any]) -> None:
    contract = _dict_value(recipe, "contract")
    runtime = _dict_value(recipe, "runtime")

    contract_id = _string_value(contract, "id")
    contract_version = _string_value(contract, "version")
    if not re.fullmatch(r"[a-z][a-z0-9-]*", contract_id):
        raise ReleaseToolError("invalid contract id")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", contract_version):
        raise ReleaseToolError("invalid contract version")

    _string_value(runtime, "python_version")
    _string_value(runtime, "platform")
    comfyui = _dict_value(runtime, "comfyui")
    _string_value(comfyui, "repository_url")
    comfyui_revision = _string_value(comfyui, "revision")
    if not re.fullmatch(r"[0-9a-f]{40}", comfyui_revision):
        raise ReleaseToolError("invalid ComfyUI revision")
    pytorch = _dict_value(runtime, "pytorch")
    _string_value(pytorch, "index_url")
    _string_list_value(pytorch, "packages")
    for requirement_path in _string_list_value(runtime, "base_requirements"):
        if not _is_safe_relative_path(requirement_path):
            raise ReleaseToolError(f"base requirement path is unsafe: {requirement_path}")


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


def _is_safe_relative_path(value: str) -> bool:
    path = Path(value)
    return bool(path.parts) and not path.is_absolute() and all(part not in ("", ".", "..") for part in path.parts)


def _validate_image_ref(value: str) -> None:
    if re.fullmatch(r"[^:@\s]+(?:/[^:@\s]+)*@sha256:[0-9a-f]{64}", value) is None:
        raise ReleaseToolError(f"worker image ref must be digest-pinned: {value}")


def _cmd_resolve(args: argparse.Namespace) -> None:
    recipe_path = Path(args.recipe)
    recipe = load_recipe(recipe_path)
    outputs = recipe_outputs(recipe, recipe_path)
    if args.github_output:
        write_github_outputs(outputs, Path(args.github_output))
    else:
        for key, value in outputs.items():
            print(f"{key}={value}")


def _cmd_validate_catalog(args: argparse.Namespace) -> None:
    validate_catalog_compatibility(recipe=load_recipe(Path(args.recipe)), catalog=_load_json(Path(args.catalog)))


def _cmd_update_catalog(args: argparse.Namespace) -> None:
    recipe = load_recipe(Path(args.recipe))
    catalog_path = Path(args.catalog)
    updated = update_catalog(
        recipe=recipe,
        catalog=_load_json(catalog_path),
        provisioner_ref=args.provisioner_ref,
        endpoint_ref=args.endpoint_ref,
    )
    _write_json(catalog_path, updated)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Runtime recipe release helper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve = subparsers.add_parser("resolve", help="resolve recipe outputs")
    resolve.add_argument("--recipe", required=True)
    resolve.add_argument("--catalog")
    resolve.add_argument("--github-output")
    resolve.set_defaults(func=_cmd_resolve)

    validate_catalog = subparsers.add_parser("validate-catalog", help="validate catalog shape")
    validate_catalog.add_argument("--recipe", required=True)
    validate_catalog.add_argument("--catalog", required=True)
    validate_catalog.set_defaults(func=_cmd_validate_catalog)

    update = subparsers.add_parser("update-catalog", help="upsert a runtime contract image pair")
    update.add_argument("--recipe", required=True)
    update.add_argument("--catalog", required=True)
    update.add_argument("--provisioner-ref", required=True)
    update.add_argument("--endpoint-ref", required=True)
    update.set_defaults(func=_cmd_update_catalog)

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
