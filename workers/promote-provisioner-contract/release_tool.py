#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


DEFAULT_PROVISIONER_CONTRACT_ID = "luma-forge-provisioner"


class ReleaseToolError(Exception):
    pass


def find_contract(catalog: dict[str, Any], contract_id: str) -> dict[str, Any] | None:
    for contract in _list_value(catalog, "contracts"):
        if not isinstance(contract, dict):
            raise ReleaseToolError("provisioner contracts contains a malformed contract entry")
        if contract.get("id") == contract_id:
            return contract
    return None


def find_provisioner_revision(contract: dict[str, Any], contract_version: str) -> dict[str, Any] | None:
    for revision in _list_value(contract, "revisions"):
        if not isinstance(revision, dict):
            raise ReleaseToolError("provisioner contracts contains a malformed revision entry")
        if revision.get("version") == contract_version:
            return revision
    return None


def next_provisioner_contract_version(*, catalog: dict[str, Any], contract_id: str) -> str:
    catalog_contract = find_contract(catalog, contract_id)
    if catalog_contract is None:
        raise ReleaseToolError(f"provisioner contracts does not contain contract: {contract_id}")

    revisions = _list_value(catalog_contract, "revisions")
    if not revisions:
        raise ReleaseToolError(f"provisioner contracts contract has no revisions: {contract_id}")

    latest = max(_parse_semver(_provisioner_revision_version(revision)) for revision in revisions)
    return _format_semver((latest[0], latest[1], latest[2] + 1))


def promote_provisioner_image(
    *,
    catalog: dict[str, Any],
    contract_id: str,
    image_ref: str,
    contract_version: str | None = None,
) -> dict[str, Any]:
    _validate_image_ref(image_ref)
    catalog_contract = find_contract(catalog, contract_id)
    if catalog_contract is None:
        raise ReleaseToolError(f"provisioner contracts does not contain contract: {contract_id}")

    revisions = _list_value(catalog_contract, "revisions")
    if not revisions:
        raise ReleaseToolError(f"provisioner contracts contract has no revisions: {contract_id}")

    resolved_contract_version = contract_version or next_provisioner_contract_version(
        catalog=catalog,
        contract_id=contract_id,
    )
    _parse_semver(resolved_contract_version)
    if find_provisioner_revision(catalog_contract, resolved_contract_version) is not None:
        raise ReleaseToolError(
            f"provisioner contracts revision already exists: {contract_id} {resolved_contract_version}"
        )

    latest_revision = max(revisions, key=lambda revision: _parse_semver(_provisioner_revision_version(revision)))
    _string_value(latest_revision, "image_ref")
    new_revision = dict(latest_revision)
    new_revision["version"] = resolved_contract_version
    new_revision["image_ref"] = image_ref
    revisions.append(new_revision)
    return catalog


def update_provisioner_workflow_catalog(
    *,
    catalog: dict[str, Any],
    contract_id: str,
    contract_version: str,
) -> dict[str, Any]:
    workflow_presets = _list_value(catalog, "workflow_presets")
    updated = False
    for preset in workflow_presets:
        if not isinstance(preset, dict):
            raise ReleaseToolError("workflow catalog contains a malformed preset entry")
        provisioner_contract = _dict_value(preset, "provisioner_contract")
        if provisioner_contract.get("id") == contract_id:
            provisioner_contract["version"] = contract_version
            updated = True
    if not updated:
        raise ReleaseToolError(f"workflow catalog does not reference provisioner contract: {contract_id}")
    return catalog


def provisioner_outputs(*, catalog: dict[str, Any], contract_id: str) -> dict[str, str]:
    return {
        "contract_id": contract_id,
        "contract_version": next_provisioner_contract_version(catalog=catalog, contract_id=contract_id),
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


def _provisioner_revision_version(revision: Any) -> str:
    if not isinstance(revision, dict):
        raise ReleaseToolError("provisioner contracts contains a malformed revision entry")
    return _string_value(revision, "version")


def _validate_image_ref(value: str) -> None:
    if re.fullmatch(r"[^:@\s]+(?:/[^:@\s]+)*@sha256:[0-9a-f]{64}", value) is None:
        raise ReleaseToolError(f"worker image ref must be digest-pinned: {value}")


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


def _cmd_resolve_provisioner(args: argparse.Namespace) -> None:
    outputs = provisioner_outputs(
        catalog=_load_json(Path(args.catalog)),
        contract_id=args.contract_id,
    )
    if args.github_output:
        write_github_outputs(outputs, Path(args.github_output))
    else:
        for key, value in outputs.items():
            print(f"{key}={value}")


def _cmd_promote_provisioner_image(args: argparse.Namespace) -> None:
    catalog_path = Path(args.catalog)
    provisioner_catalog = _load_json(catalog_path)
    contract_version = args.contract_version or next_provisioner_contract_version(
        catalog=provisioner_catalog,
        contract_id=args.contract_id,
    )
    updated = promote_provisioner_image(
        catalog=provisioner_catalog,
        contract_id=args.contract_id,
        image_ref=args.image_ref,
        contract_version=contract_version,
    )
    updated_workflow_catalog = None
    if args.workflow_catalog:
        workflow_catalog_path = Path(args.workflow_catalog)
        updated_workflow_catalog = update_provisioner_workflow_catalog(
            catalog=_load_json(workflow_catalog_path),
            contract_id=args.contract_id,
            contract_version=contract_version,
        )
    _write_json(catalog_path, updated)
    if args.workflow_catalog and updated_workflow_catalog is not None:
        _write_json(Path(args.workflow_catalog), updated_workflow_catalog)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Provisioner contract image promotion helper")
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve_provisioner = subparsers.add_parser("resolve-provisioner", help="resolve provisioner contracts outputs")
    resolve_provisioner.add_argument("--catalog", required=True)
    resolve_provisioner.add_argument("--contract-id", default=DEFAULT_PROVISIONER_CONTRACT_ID)
    resolve_provisioner.add_argument("--github-output")
    resolve_provisioner.set_defaults(func=_cmd_resolve_provisioner)

    promote_provisioner = subparsers.add_parser(
        "promote-provisioner-image",
        help="promote a digest-pinned provisioner image into the Provisioner Contracts",
    )
    promote_provisioner.add_argument("--catalog", required=True)
    promote_provisioner.add_argument("--image-ref", required=True)
    promote_provisioner.add_argument("--contract-id", default=DEFAULT_PROVISIONER_CONTRACT_ID)
    promote_provisioner.add_argument("--contract-version")
    promote_provisioner.add_argument("--workflow-catalog")
    promote_provisioner.set_defaults(func=_cmd_promote_provisioner_image)

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
