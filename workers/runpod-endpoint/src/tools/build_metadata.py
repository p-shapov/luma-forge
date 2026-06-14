from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


class BuildMetadataError(Exception):
    pass


def extract_runtime_metadata(
    *,
    workflow_catalog_path: Path,
    execution_schemas_path: Path,
    workflow_id: str,
    workflow_version: str,
    execution_contract_output_path: Path,
) -> None:
    workflow_catalog = _load_json(workflow_catalog_path)
    execution_schemas = _load_json(execution_schemas_path)
    revision = _find_workflow_revision(workflow_catalog, workflow_id, workflow_version)
    catalog_execution_contract = _dict_value(revision, "execution_contract")
    schema_ref = _dict_value(catalog_execution_contract, "schema_ref")
    schema_id = _string_value(schema_ref, "id")
    schema_version = _string_value(schema_ref, "version")
    execution_schema_revision = _find_execution_schema_revision(execution_schemas, schema_id, schema_version)
    execution_contract = {
        "execution_schema": execution_schema_revision,
        "input_bindings": _list_value(catalog_execution_contract, "input_bindings"),
    }

    execution_contract_output_path.parent.mkdir(parents=True, exist_ok=True)
    _write_json(execution_contract_output_path, execution_contract)


def _find_workflow_revision(catalog: dict[str, Any], workflow_id: str, workflow_version: str) -> dict[str, Any]:
    for preset in _list_value(catalog, "workflow_presets"):
        if isinstance(preset, dict) and preset.get("id") == workflow_id:
            for revision in _list_value(preset, "revisions"):
                if isinstance(revision, dict) and revision.get("version") == workflow_version:
                    return revision
    raise BuildMetadataError(f"workflow revision was not found: {workflow_id} {workflow_version}")


def _find_execution_schema_revision(registry: dict[str, Any], schema_id: str, schema_version: str) -> dict[str, Any]:
    for schema in _list_value(registry, "execution_schemas"):
        if isinstance(schema, dict) and schema.get("id") == schema_id:
            for revision in _list_value(schema, "revisions"):
                if isinstance(revision, dict) and revision.get("version") == schema_version:
                    return revision
    raise BuildMetadataError(f"execution schema revision was not found: {schema_id} {schema_version}")


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise BuildMetadataError(f"{path} must contain a JSON object")
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def _dict_value(value: dict[str, Any], key: str) -> dict[str, Any]:
    item = value.get(key)
    if not isinstance(item, dict):
        raise BuildMetadataError(f"{key} must be an object")
    return item


def _list_value(value: dict[str, Any], key: str) -> list[Any]:
    item = value.get(key)
    if not isinstance(item, list):
        raise BuildMetadataError(f"{key} must be a list")
    return item


def _string_value(value: dict[str, Any], key: str) -> str:
    item = value.get(key)
    if not isinstance(item, str) or item.strip() == "":
        raise BuildMetadataError(f"{key} must be a non-empty string")
    return item


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workflow-catalog", required=True)
    parser.add_argument("--execution-schemas", required=True)
    parser.add_argument("--workflow-id", required=True)
    parser.add_argument("--workflow-version", required=True)
    parser.add_argument("--execution-contract-output", required=True)
    args = parser.parse_args()
    extract_runtime_metadata(
        workflow_catalog_path=Path(args.workflow_catalog),
        execution_schemas_path=Path(args.execution_schemas),
        workflow_id=args.workflow_id,
        workflow_version=args.workflow_version,
        execution_contract_output_path=Path(args.execution_contract_output),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
