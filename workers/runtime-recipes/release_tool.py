#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import urlparse


ENVIRONMENT_KIND = "image_baked_comfyui_runtime"
BASE_DEPENDENCY_RECORD_PATHS = [
    ".luma-forge/base-runtime/pip-freeze.txt",
    ".luma-forge/base-runtime/install-report.json",
]
PROVISIONER_RUNTIME_ARCHIVE_PATH = "/opt/luma-forge/runtime/base-runtime.tar.gz"
PROVISIONER_RUNTIME_METADATA_PATH = "/opt/luma-forge/runtime/runtime-metadata.json"
ENDPOINT_RUNTIME_CONTRACT_PATH = "/opt/luma-forge/runtime/runtime-contract.json"


class ReleaseToolError(Exception):
    pass


def load_recipe(path: Path) -> dict[str, Any]:
    recipe = _load_yaml(path)
    schema = _load_json(path.parent / "schema.json")
    _validate_with_schema(recipe, schema)
    _validate_recipe(recipe)
    return recipe


def runtime_metadata_from_recipe(recipe: dict[str, Any]) -> dict[str, Any]:
    compatibility = compatibility_metadata_from_recipe(recipe)
    return {
        "environment_kind": ENVIRONMENT_KIND,
        "python_version": compatibility["python_version"],
        "platform": compatibility["platform"],
        "comfyui_revision": compatibility["comfyui_revision"],
        "base_dependency_record_paths": list(BASE_DEPENDENCY_RECORD_PATHS),
        "runtime_compatibility": {
            "pytorch_index_url": compatibility["pytorch_index_url"],
            "pytorch_packages": list(compatibility["pytorch_packages"]),
            "base_requirements": list(compatibility["base_requirements"]),
        },
    }


def compatibility_metadata_from_recipe(recipe: dict[str, Any]) -> dict[str, Any]:
    runtime = recipe["runtime"]
    return {
        "environment_kind": ENVIRONMENT_KIND,
        "python_version": runtime["python_version"],
        "platform": runtime["platform"],
        "comfyui_revision": runtime["comfyui"]["revision"],
        "base_dependency_record_paths": list(BASE_DEPENDENCY_RECORD_PATHS),
        "pytorch_index_url": runtime["pytorch"]["index_url"],
        "pytorch_packages": list(runtime["pytorch"]["packages"]),
        "base_requirements": list(runtime["base_requirements"]),
    }


def compatibility_metadata_from_contract(contract: dict[str, Any]) -> dict[str, Any]:
    runtime_metadata = _dict_value(contract, "runtime_metadata")
    runtime_compatibility = runtime_metadata.get("runtime_compatibility")
    if not isinstance(runtime_compatibility, dict):
        runtime_compatibility = {}
    return {
        "environment_kind": runtime_metadata.get("environment_kind"),
        "python_version": runtime_metadata.get("python_version"),
        "platform": runtime_metadata.get("platform"),
        "comfyui_revision": runtime_metadata.get("comfyui_revision"),
        "base_dependency_record_paths": runtime_metadata.get("base_dependency_record_paths"),
        "pytorch_index_url": runtime_compatibility.get("pytorch_index_url"),
        "pytorch_packages": runtime_compatibility.get("pytorch_packages"),
        "base_requirements": runtime_compatibility.get("base_requirements"),
    }


def find_contract(catalog: dict[str, Any], contract_id: str, contract_version: str) -> dict[str, Any] | None:
    contracts = _list_value(catalog, "runtime_contracts")
    for contract in contracts:
        if not isinstance(contract, dict):
            raise ReleaseToolError("runtime catalog contains a malformed contract entry")
        if contract.get("id") == contract_id and contract.get("version") == contract_version:
            return contract
    return None


def validate_catalog_compatibility(
    *,
    recipe: dict[str, Any],
    catalog: dict[str, Any],
    implementation_revision: str,
) -> None:
    contract_id = recipe["contract"]["id"]
    contract_version = recipe["contract"]["version"]
    contract = find_contract(catalog, contract_id, contract_version)
    if contract is None:
        return

    revisions = _list_value(contract, "implementation_revisions")
    if any(isinstance(item, dict) and item.get("revision") == implementation_revision for item in revisions):
        raise ReleaseToolError(f"implementation revision already exists: {implementation_revision}")

    expected = compatibility_metadata_from_recipe(recipe)
    actual = compatibility_metadata_from_contract(contract)
    mismatches = [key for key in expected if actual.get(key) != expected[key]]
    if mismatches:
        details = ", ".join(mismatches)
        raise ReleaseToolError(
            "runtime contract compatibility mismatch for "
            f"{contract_id} {contract_version}: {details}. "
            "Bump the runtime contract version or restore the recipe to the existing compatibility surface."
        )


def update_catalog(
    *,
    recipe: dict[str, Any],
    catalog: dict[str, Any],
    implementation_revision: str,
    provisioner_ref: str,
    endpoint_ref: str,
) -> dict[str, Any]:
    validate_catalog_compatibility(
        recipe=recipe,
        catalog=catalog,
        implementation_revision=implementation_revision,
    )

    contracts = _list_value(catalog, "runtime_contracts")
    contract_id = recipe["contract"]["id"]
    contract_version = recipe["contract"]["version"]
    contract = find_contract(catalog, contract_id, contract_version)
    if contract is None:
        contract = {
            "id": contract_id,
            "version": contract_version,
            "display_name": f"{contract_id} {contract_version}",
            "runtime_metadata": runtime_metadata_from_recipe(recipe),
            "implementation_revisions": [],
            "default_implementation_revision": implementation_revision,
        }
        contracts.append(contract)

    contract["implementation_revisions"].append(
        {
            "revision": implementation_revision,
            "provisioner_image_ref": provisioner_ref,
            "endpoint_image_ref": endpoint_ref,
            "image_metadata": {
                "provisioner_runtime_archive_path": PROVISIONER_RUNTIME_ARCHIVE_PATH,
                "provisioner_runtime_metadata_path": PROVISIONER_RUNTIME_METADATA_PATH,
                "endpoint_runtime_contract_path": ENDPOINT_RUNTIME_CONTRACT_PATH,
            },
        }
    )
    contract["default_implementation_revision"] = implementation_revision
    return catalog


def validate_image_metadata(
    *,
    recipe: dict[str, Any],
    implementation_revision: str,
    provisioner_metadata: dict[str, Any],
    endpoint_metadata: dict[str, Any],
) -> None:
    contract = recipe["contract"]
    compatibility = compatibility_metadata_from_recipe(recipe)
    expected_identity = {
        "contract_id": contract["id"],
        "contract_version": contract["version"],
        "implementation_revision": implementation_revision,
    }
    for key, value in expected_identity.items():
        if provisioner_metadata.get(key) != value:
            raise ReleaseToolError(f"provisioner runtime metadata mismatch: {key}")
        if endpoint_metadata.get(key) != value:
            raise ReleaseToolError(f"endpoint runtime metadata mismatch: {key}")

    expected_provisioner = {
        "python_version": compatibility["python_version"],
        "platform": compatibility["platform"],
        "comfyui_revision": compatibility["comfyui_revision"],
        "pytorch_index_url": compatibility["pytorch_index_url"],
        "pytorch_packages": compatibility["pytorch_packages"],
        "base_requirements": compatibility["base_requirements"],
    }
    mismatches = [
        key for key, value in expected_provisioner.items() if provisioner_metadata.get(key) != value
    ]
    if mismatches:
        raise ReleaseToolError(
            "provisioner runtime metadata does not match selected recipe: "
            + ", ".join(mismatches)
        )


def recipe_outputs(recipe: dict[str, Any], recipe_path: Path, implementation_revision: str) -> dict[str, str]:
    runtime = recipe["runtime"]
    packages_json = json.dumps(runtime["pytorch"]["packages"], separators=(",", ":"))
    requirements_json = json.dumps(runtime["base_requirements"], separators=(",", ":"))
    return {
        "recipe": str(recipe_path),
        "contract_id": recipe["contract"]["id"],
        "contract_version": recipe["contract"]["version"],
        "implementation_revision": implementation_revision,
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
                raise ReleaseToolError(
                    f"recipe schema validation failed at {path}.{extra[0]}: unsupported property"
                )
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
            raise ReleaseToolError(
                f"recipe schema validation failed at {path}: expected at least {min_length} characters"
            )
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
        if current_indent < indent:
            break
        if current_indent != indent or text.startswith("- "):
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
    _dict_value(recipe, "metadata")

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


def _cmd_resolve(args: argparse.Namespace) -> None:
    recipe_path = Path(args.recipe)
    recipe = load_recipe(recipe_path)
    outputs = recipe_outputs(recipe, recipe_path, args.implementation_revision)
    if args.github_output:
        write_github_outputs(outputs, Path(args.github_output))
    else:
        for key, value in outputs.items():
            print(f"{key}={value}")


def _cmd_validate_catalog(args: argparse.Namespace) -> None:
    recipe = load_recipe(Path(args.recipe))
    catalog = _load_json(Path(args.catalog))
    validate_catalog_compatibility(
        recipe=recipe,
        catalog=catalog,
        implementation_revision=args.implementation_revision,
    )


def _cmd_update_catalog(args: argparse.Namespace) -> None:
    recipe = load_recipe(Path(args.recipe))
    catalog_path = Path(args.catalog)
    catalog = _load_json(catalog_path)
    updated = update_catalog(
        recipe=recipe,
        catalog=catalog,
        implementation_revision=args.implementation_revision,
        provisioner_ref=args.provisioner_ref,
        endpoint_ref=args.endpoint_ref,
    )
    _write_json(catalog_path, updated)


def _cmd_validate_image_metadata(args: argparse.Namespace) -> None:
    recipe = load_recipe(Path(args.recipe))
    validate_image_metadata(
        recipe=recipe,
        implementation_revision=args.implementation_revision,
        provisioner_metadata=_load_json(Path(args.provisioner_runtime_metadata)),
        endpoint_metadata=_load_json(Path(args.endpoint_runtime_metadata)),
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Runtime recipe release helper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve = subparsers.add_parser("resolve", help="resolve recipe outputs")
    resolve.add_argument("--recipe", required=True)
    resolve.add_argument("--implementation-revision", required=True)
    resolve.add_argument("--github-output")
    resolve.set_defaults(func=_cmd_resolve)

    validate_catalog = subparsers.add_parser("validate-catalog", help="validate catalog compatibility")
    validate_catalog.add_argument("--recipe", required=True)
    validate_catalog.add_argument("--catalog", required=True)
    validate_catalog.add_argument("--implementation-revision", required=True)
    validate_catalog.set_defaults(func=_cmd_validate_catalog)

    update = subparsers.add_parser("update-catalog", help="append an implementation revision")
    update.add_argument("--recipe", required=True)
    update.add_argument("--catalog", required=True)
    update.add_argument("--implementation-revision", required=True)
    update.add_argument("--provisioner-ref", required=True)
    update.add_argument("--endpoint-ref", required=True)
    update.set_defaults(func=_cmd_update_catalog)

    validate_image = subparsers.add_parser("validate-image-metadata", help="validate built image metadata")
    validate_image.add_argument("--recipe", required=True)
    validate_image.add_argument("--implementation-revision", required=True)
    validate_image.add_argument("--provisioner-runtime-metadata", required=True)
    validate_image.add_argument("--endpoint-runtime-metadata", required=True)
    validate_image.set_defaults(func=_cmd_validate_image_metadata)
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
