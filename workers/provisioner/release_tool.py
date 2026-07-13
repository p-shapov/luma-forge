#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT))

from workers.catalog_fs import (  # noqa: E402
    ReleaseToolError,
    dict_value,
    ensure_destination_available,
    is_safe_identifier,
    latest_revision,
    load_json,
    next_revision,
    parse_semver,
    runpod_contract_requirements,
    validate_image_ref,
    validate_workflow_revision,
    write_json,
)


DEFAULT_PROVISIONER_CONTRACT_ID = "provisioner"


def next_provisioner_contract_revision(
    catalog_root: Path, contract_id: str
) -> str:
    if not is_safe_identifier(contract_id):
        raise ReleaseToolError("invalid runtime contract id")
    return next_revision(
        catalog_root / "entries/runtime_contracts" / contract_id,
    )


def promote_provisioner_image(
    *,
    catalog_root: Path,
    contract_id: str,
    contract_revision: str,
    image_ref: str,
) -> tuple[Path, list[Path]]:
    validate_image_ref(image_ref)
    if not is_safe_identifier(contract_id):
        raise ReleaseToolError("invalid runtime contract id")
    parse_semver(contract_revision)
    contract_dir = (
        catalog_root / "entries/runtime_contracts" / contract_id / contract_revision
    )
    ensure_destination_available(contract_dir)

    promotions: list[tuple[Path, Path, dict[str, Any]]] = []
    workflows_root = catalog_root / "entries/workflows"
    for workflow_root in sorted(
        path for path in workflows_root.iterdir() if path.is_dir()
    ):
        if not is_safe_identifier(workflow_root.name):
            raise ReleaseToolError("invalid workflow id")
        _source_revision, source = latest_revision(workflow_root)
        validate_workflow_revision(source)
        requirements = load_json(source / "contract_requirements")
        runpod = runpod_contract_requirements(requirements)
        reference = dict_value(runpod, "provisioner_contract_ref")
        if reference.get("id") != contract_id:
            continue
        destination = workflow_root / next_revision(workflow_root)
        ensure_destination_available(destination)
        reference["revision"] = contract_revision
        promotions.append((source, destination, requirements))

    if not promotions:
        raise ReleaseToolError(
            f"workflow catalog does not reference provisioner contract: {contract_id}"
        )

    contract_dir.mkdir(parents=True)
    write_json(contract_dir / "runtime_contract", {"image_ref": image_ref})
    destinations = []
    for source, destination, requirements in promotions:
        shutil.copytree(source, destination)
        write_json(destination / "contract_requirements", requirements)
        destinations.append(destination)
    return contract_dir / "runtime_contract", destinations


def provisioner_outputs(*, catalog_root: Path, contract_id: str) -> dict[str, str]:
    return {
        "contract_id": contract_id,
        "contract_revision": next_provisioner_contract_revision(
            catalog_root, contract_id
        ),
    }


def write_github_outputs(outputs: dict[str, str], path: Path) -> None:
    with path.open("a", encoding="utf-8") as handle:
        for key, value in outputs.items():
            if "\n" in value or "\r" in value:
                raise ReleaseToolError(f"output contains newline: {key}")
            handle.write(f"{key}={value}\n")


def _write_outputs(outputs: dict[str, str], github_output: str | None) -> None:
    if github_output:
        write_github_outputs(outputs, Path(github_output))
    else:
        for key, value in outputs.items():
            print(f"{key}={value}")


def _cmd_resolve_provisioner(args: argparse.Namespace) -> None:
    _write_outputs(
        provisioner_outputs(
            catalog_root=Path(args.catalog_root),
            contract_id=args.contract_id,
        ),
        args.github_output,
    )


def _cmd_promote_provisioner_image(args: argparse.Namespace) -> None:
    contract_path, _workflow_paths = promote_provisioner_image(
        catalog_root=Path(args.catalog_root),
        contract_id=args.contract_id,
        contract_revision=args.contract_revision,
        image_ref=args.image_ref,
    )
    _write_outputs(
        {"runtime_contract_path": str(contract_path)},
        args.github_output,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Provisioner contract image promotion helper"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve_provisioner = subparsers.add_parser(
        "resolve-provisioner", help="resolve provisioner runtime contract outputs"
    )
    resolve_provisioner.add_argument("--catalog-root", required=True)
    resolve_provisioner.add_argument(
        "--contract-id", default=DEFAULT_PROVISIONER_CONTRACT_ID
    )
    resolve_provisioner.add_argument("--github-output")
    resolve_provisioner.set_defaults(func=_cmd_resolve_provisioner)

    promote_provisioner = subparsers.add_parser(
        "promote-provisioner-image",
        help="promote a digest-pinned provisioner image into catalog revisions",
    )
    promote_provisioner.add_argument("--catalog-root", required=True)
    promote_provisioner.add_argument("--image-ref", required=True)
    promote_provisioner.add_argument(
        "--contract-id", default=DEFAULT_PROVISIONER_CONTRACT_ID
    )
    promote_provisioner.add_argument("--contract-revision", required=True)
    promote_provisioner.add_argument("--github-output")
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
