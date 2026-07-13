import argparse
import json
from pathlib import Path
from typing import Any


def extract_runtime_metadata(
    *,
    execution_contract_path: Path,
    execution_schema_path: Path,
    execution_contract_output_path: Path,
) -> None:
    source = _load_json(execution_contract_path)
    output = {
        "execution_schema": _load_json(execution_schema_path),
        "input_bindings": _list_value(source, "input_bindings"),
    }
    execution_contract_output_path.parent.mkdir(parents=True, exist_ok=True)
    _write_json(execution_contract_output_path, output)


def _load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def _list_value(value: dict[str, Any], key: str) -> list[Any]:
    item = value.get(key)
    if not isinstance(item, list):
        raise ValueError(f"{key} must be a list")
    return item


def main() -> None:
    parser = argparse.ArgumentParser(description="Extract RunPod runtime metadata")
    parser.add_argument("--execution-contract", required=True)
    parser.add_argument("--execution-schema", required=True)
    parser.add_argument("--execution-contract-output", required=True)
    args = parser.parse_args()
    extract_runtime_metadata(
        execution_contract_path=Path(args.execution_contract),
        execution_schema_path=Path(args.execution_schema),
        execution_contract_output_path=Path(args.execution_contract_output),
    )


if __name__ == "__main__":
    main()
