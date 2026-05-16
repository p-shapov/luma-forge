from dataclasses import asdict, dataclass
from datetime import UTC, datetime
import json
import platform
from pathlib import Path

from app.errors import PreparationError
from auxiliary.paths import safe_child_path
from app.schemas import CustomNode, StartRequest

ENVIRONMENT_KIND = "image_baked_comfyui_runtime"
METADATA_DIR = ".luma-forge"
RUNTIME_MANIFEST = "runtime.json"
PIP_FREEZE = "pip-freeze.txt"
INSTALL_REPORT = "install-report.json"
VENV_DIR = ".venv"


@dataclass(frozen=True)
class RuntimePaths:
    workspace_root: Path
    comfyui_root: Path
    metadata_dir: Path
    venv_dir: Path
    python_path: Path
    runtime_manifest_path: Path
    pip_freeze_path: Path
    install_report_path: Path


@dataclass(frozen=True)
class PreparedRuntimeManifest:
    environment_kind: str
    python_path: str
    comfyui_root: str
    python_version: str
    platform: str
    comfyui_revision: str
    runtime_contract_id: str
    runtime_contract_version: str
    implementation_revision: str
    provisioner_image_ref: str
    endpoint_image_ref: str
    custom_node_revisions: list[dict[str, str]]
    base_dependency_record_paths: list[str]
    prepared_at: str

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, sort_keys=True) + "\n"


def runtime_paths(workspace_root: Path) -> RuntimePaths:
    workspace = workspace_root.resolve(strict=False)
    metadata_dir = safe_child_path(workspace, METADATA_DIR, field_name="runtime_metadata_path")
    venv_dir = safe_child_path(workspace, VENV_DIR, field_name="volume_virtual_environment_path")
    return RuntimePaths(
        workspace_root=workspace,
        comfyui_root=workspace / "ComfyUI",
        metadata_dir=metadata_dir,
        venv_dir=venv_dir,
        python_path=venv_dir / "bin" / "python",
        runtime_manifest_path=metadata_dir / RUNTIME_MANIFEST,
        pip_freeze_path=metadata_dir / PIP_FREEZE,
        install_report_path=metadata_dir / INSTALL_REPORT,
    )


def build_manifest(
    *,
    request: StartRequest,
    paths: RuntimePaths,
    python_version: str,
) -> PreparedRuntimeManifest:
    return PreparedRuntimeManifest(
        environment_kind=ENVIRONMENT_KIND,
        python_path=str(paths.python_path),
        comfyui_root=str(paths.comfyui_root),
        python_version=python_version.strip(),
        platform=request.resolved_runtime_implementation.runtime_metadata.platform or platform.platform(),
        comfyui_revision=request.resolved_runtime_implementation.runtime_metadata.comfyui_revision,
        runtime_contract_id=request.resolved_runtime_implementation.contract_id,
        runtime_contract_version=request.resolved_runtime_implementation.contract_version,
        implementation_revision=request.resolved_runtime_implementation.implementation_revision,
        provisioner_image_ref=request.resolved_runtime_implementation.provisioner_image_ref,
        endpoint_image_ref=request.resolved_runtime_implementation.endpoint_image_ref,
        custom_node_revisions=_custom_node_revisions(request.workflow_preset.required_custom_nodes),
        base_dependency_record_paths=[
            str((paths.workspace_root / path).resolve(strict=False))
            for path in request.resolved_runtime_implementation.runtime_metadata.base_dependency_record_paths
        ],
        prepared_at=datetime.now(UTC).replace(microsecond=0).isoformat(),
    )


def write_manifest(manifest: PreparedRuntimeManifest, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(manifest.to_json(), encoding="utf-8")


def load_manifest(path: Path) -> PreparedRuntimeManifest:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            raise ValueError("runtime manifest must be an object")
        return PreparedRuntimeManifest(
            environment_kind=_required_string(payload, "environment_kind"),
            python_path=_required_string(payload, "python_path"),
            comfyui_root=_required_string(payload, "comfyui_root"),
            python_version=_required_string(payload, "python_version"),
            platform=_required_string(payload, "platform"),
            comfyui_revision=_required_string(payload, "comfyui_revision"),
            runtime_contract_id=_required_string(payload, "runtime_contract_id"),
            runtime_contract_version=_required_string(payload, "runtime_contract_version"),
            implementation_revision=_required_string(payload, "implementation_revision"),
            provisioner_image_ref=_required_string(payload, "provisioner_image_ref"),
            endpoint_image_ref=_required_string(payload, "endpoint_image_ref"),
            custom_node_revisions=_custom_node_revision_payload(payload),
            base_dependency_record_paths=_string_list(payload, "base_dependency_record_paths"),
            prepared_at=_required_string(payload, "prepared_at"),
        )
    except OSError as error:
        raise PreparationError("Prepared runtime manifest is missing.") from error
    except (json.JSONDecodeError, TypeError, ValueError) as error:
        raise PreparationError("Prepared runtime manifest is invalid.") from error


def validate_manifest(manifest: PreparedRuntimeManifest, *, paths: RuntimePaths) -> None:
    if manifest.environment_kind != ENVIRONMENT_KIND:
        raise PreparationError("Prepared runtime environment kind is invalid.")
    if Path(manifest.python_path).resolve(strict=False) != paths.python_path.resolve(strict=False):
        raise PreparationError("Prepared runtime Python path is invalid.")
    if Path(manifest.comfyui_root).resolve(strict=False) != paths.comfyui_root.resolve(strict=False):
        raise PreparationError("Prepared runtime ComfyUI path is invalid.")
    if not manifest.runtime_contract_id or not manifest.implementation_revision:
        raise PreparationError("Prepared runtime contract metadata is invalid.")


def _custom_node_revisions(custom_nodes: list[CustomNode]) -> list[dict[str, str]]:
    return [
        {
            "id": node.id,
            "revision": node.git_source.revision,
        }
        for node in custom_nodes
    ]


def _required_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or value.strip() == "":
        raise ValueError(f"{key} is required")
    return value


def _custom_node_revision_payload(payload: dict[str, object]) -> list[dict[str, str]]:
    value = payload.get("custom_node_revisions")
    if not isinstance(value, list):
        raise ValueError("custom_node_revisions is required")
    revisions = []
    for entry in value:
        if not isinstance(entry, dict):
            raise ValueError("custom_node_revisions entry must be an object")
        revisions.append(
            {
                "id": _required_string(entry, "id"),
                "revision": _required_string(entry, "revision"),
            }
        )
    return revisions


def _string_list(payload: dict[str, object], key: str) -> list[str]:
    value = payload.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) and item.strip() for item in value):
        raise ValueError(f"{key} is required")
    return value
