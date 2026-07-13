#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPOSITORY_ROOT))

from workers.catalog_fs import (  # noqa: E402
    ReleaseToolError,
    WORKFLOW_FILES,
    catalog_ref,
    dict_value,
    entry_file,
    is_safe_identifier,
    load_json,
    next_revision,
    parse_semver,
    runpod_contract_requirements,
    string_list_value,
    string_value,
    validate_image_ref,
    write_json,
)


def endpoint_contract_id(workflow_id: str) -> str:
    if not is_safe_identifier(workflow_id):
        raise ReleaseToolError("invalid workflow id")
    return f"runpod-endpoint-{workflow_id}"


def resolve_endpoint_build(
    *, catalog_root: Path, workflow_id: str, workflow_revision: str
) -> dict[str, str]:
    workflow_dir = entry_file(
        catalog_root, "workflow", workflow_id, workflow_revision, "workflow"
    ).parent
    metadata = load_json(workflow_dir / "metadata")
    execution_contract_path = workflow_dir / "execution_contract"
    execution_contract = load_json(execution_contract_path)

    preset_id, preset_revision = catalog_ref(
        metadata,
        "runtime_preset_ref",
        "catalog/contracts/runtime_preset_revision",
    )
    schema_id, schema_revision = catalog_ref(
        execution_contract,
        "schema_ref",
        "catalog/contracts/execution_schema_revision",
    )
    runtime_preset_path = entry_file(
        catalog_root, "runtime_preset", preset_id, preset_revision, "runtime_preset"
    )
    execution_schema_path = entry_file(
        catalog_root, "execution_schema", schema_id, schema_revision, "execution_schema"
    )
    runtime = dict_value(load_json(runtime_preset_path), "runtime")
    pytorch = dict_value(runtime, "pytorch")
    contract_id = endpoint_contract_id(workflow_id)
    contract_root = catalog_root / "entries/runtime_contracts" / contract_id
    comfyui_revision = string_value(runtime, "comfyui_revision")
    if not re.fullmatch(r"[0-9a-f]{40}", comfyui_revision):
        raise ReleaseToolError("invalid ComfyUI revision")

    return {
        "workflow_path": str(workflow_dir / "workflow"),
        "execution_contract_path": str(execution_contract_path),
        "execution_schema_path": str(execution_schema_path),
        "runtime_preset_path": str(runtime_preset_path),
        "workflow_id": workflow_id,
        "workflow_revision": workflow_revision,
        "contract_id": contract_id,
        "contract_revision": next_revision(contract_root, initial="1.0.0"),
        "runtime_python_version": string_value(runtime, "python_version"),
        "comfyui_revision": comfyui_revision,
        "pytorch_index_url": string_value(pytorch, "index_url"),
        "pytorch_packages_json": json.dumps(
            string_list_value(pytorch, "packages"), separators=(",", ":")
        ),
    }


def promote_endpoint_image(
    *,
    catalog_root: Path,
    workflow_id: str,
    workflow_revision: str,
    contract_revision: str,
    image_ref: str,
) -> tuple[Path, Path]:
    validate_image_ref(image_ref)
    contract_id = endpoint_contract_id(workflow_id)
    parse_semver(contract_revision)
    source = entry_file(
        catalog_root, "workflow", workflow_id, workflow_revision, "workflow"
    ).parent
    requirements_path = source / "contract_requirements"
    requirements = load_json(requirements_path)
    runpod = runpod_contract_requirements(requirements)
    endpoint_ref = dict_value(runpod, "endpoint_contract_ref")
    if endpoint_ref.get("id") != contract_id:
        raise ReleaseToolError(
            f"workflow revision does not reference endpoint contract: {contract_id}"
        )

    contract_dir = (
        catalog_root / "entries/runtime_contracts" / contract_id / contract_revision
    )
    workflow_root = catalog_root / "entries/workflows" / workflow_id
    promoted_revision = next_revision(workflow_root)
    promoted_dir = workflow_root / promoted_revision
    if contract_dir.exists() or promoted_dir.exists():
        raise ReleaseToolError("catalog promotion revision already exists")
    for name in WORKFLOW_FILES:
        if not (source / name).is_file():
            raise ReleaseToolError(
                f"workflow revision file does not exist: {source / name}"
            )

    endpoint_ref["revision"] = contract_revision
    contract_dir.mkdir(parents=True)
    write_json(contract_dir / "runtime_contract", {"image_ref": image_ref})
    shutil.copytree(source, promoted_dir)
    write_json(promoted_dir / "contract_requirements", requirements)
    return contract_dir / "runtime_contract", promoted_dir


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


def _cmd_resolve(args: argparse.Namespace) -> None:
    _write_outputs(
        resolve_endpoint_build(
            catalog_root=Path(args.catalog_root),
            workflow_id=args.workflow_id,
            workflow_revision=args.workflow_revision,
        ),
        args.github_output,
    )


def _cmd_promote_endpoint_image(args: argparse.Namespace) -> None:
    contract_path, workflow_path = promote_endpoint_image(
        catalog_root=Path(args.catalog_root),
        workflow_id=args.workflow_id,
        workflow_revision=args.workflow_revision,
        contract_revision=args.contract_revision,
        image_ref=args.image_ref,
    )
    _write_outputs(
        {
            "runtime_contract_path": str(contract_path),
            "workflow_revision_path": str(workflow_path),
            "promoted_workflow_revision": workflow_path.name,
        },
        args.github_output,
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="RunPod endpoint release and promotion helper"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    resolve = subparsers.add_parser(
        "resolve", help="resolve workflow endpoint build outputs"
    )
    resolve.add_argument("--catalog-root", required=True)
    resolve.add_argument("--workflow-id", required=True)
    resolve.add_argument("--workflow-revision", required=True)
    resolve.add_argument("--github-output")
    resolve.set_defaults(func=_cmd_resolve)

    promote_endpoint = subparsers.add_parser(
        "promote-endpoint-image",
        help="promote a digest-pinned RunPod endpoint image into catalog revisions",
    )
    promote_endpoint.add_argument("--catalog-root", required=True)
    promote_endpoint.add_argument("--workflow-id", required=True)
    promote_endpoint.add_argument("--workflow-revision", required=True)
    promote_endpoint.add_argument("--contract-revision", required=True)
    promote_endpoint.add_argument("--image-ref", required=True)
    promote_endpoint.add_argument("--github-output")
    promote_endpoint.set_defaults(func=_cmd_promote_endpoint_image)

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
