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
RUNTIME_MANIFEST = "runtime-manifest.json"
PIP_FREEZE = "pip-freeze.txt"
INSTALL_REPORT = "install-report.json"
PYTHON_OVERLAY_DIR = ".luma-forge/python-overlay"
IMAGE_RUNTIME_ROOT = Path("/opt/luma-forge/runtime")
IMAGE_PYTHON_PATH = IMAGE_RUNTIME_ROOT / ".venv" / "bin" / "python"
IMAGE_COMFYUI_ROOT = IMAGE_RUNTIME_ROOT / "ComfyUI"


@dataclass(frozen=True)
class RuntimePaths:
    workspace_root: Path
    comfyui_root: Path
    metadata_dir: Path
    image_runtime_root: Path
    image_comfyui_root: Path
    image_python_path: Path
    python_overlay_path: Path
    python_path: Path
    runtime_manifest_path: Path
    pip_freeze_path: Path
    install_report_path: Path


@dataclass(frozen=True)
class PreparedRuntimeManifest:
    environment_kind: str
    python_path: str
    comfyui_root: str
    image_runtime_root: str
    workspace_root: str
    python_overlay_path: str
    custom_node_revisions: list[dict[str, str]]
    overlay_dependency_record_paths: list[str]
    model_asset_paths: list[str]
    prepared_at: str

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, sort_keys=True) + "\n"


def runtime_paths(
    workspace_root: Path,
    image_runtime_root: Path | None = None,
    python_overlay_path: Path | None = None,
) -> RuntimePaths:
    workspace = workspace_root.resolve(strict=False)
    metadata_dir = safe_child_path(workspace, METADATA_DIR, field_name="runtime_metadata_path")
    image_root = (image_runtime_root or IMAGE_RUNTIME_ROOT).resolve(strict=False)
    image_python = (image_root / ".venv" / "bin" / "python").resolve(strict=False)
    image_comfyui = (image_root / "ComfyUI").resolve(strict=False)
    overlay = safe_child_path(
        workspace,
        (python_overlay_path or Path(PYTHON_OVERLAY_DIR)).as_posix(),
        field_name="python_overlay_path",
    )
    return RuntimePaths(
        workspace_root=workspace,
        comfyui_root=workspace,
        metadata_dir=metadata_dir,
        image_runtime_root=image_root,
        image_comfyui_root=image_comfyui,
        image_python_path=image_python,
        python_overlay_path=overlay,
        python_path=image_python,
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
        python_path=str(paths.image_python_path),
        comfyui_root=str(paths.image_comfyui_root),
        image_runtime_root=str(paths.image_runtime_root),
        workspace_root=str(paths.workspace_root),
        python_overlay_path=str(paths.python_overlay_path),
        custom_node_revisions=_custom_node_revisions(request.workflow_preset.required_custom_nodes),
        overlay_dependency_record_paths=[
            str(path)
            for path in sorted(paths.metadata_dir.glob("custom-node-*-install-report.json"))
        ],
        model_asset_paths=[
            str((paths.workspace_root / asset.install.comfyui_relative_path).resolve(strict=False))
            for asset in request.workflow_preset.required_model_assets
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
            image_runtime_root=_required_string(payload, "image_runtime_root"),
            workspace_root=_required_string(payload, "workspace_root"),
            python_overlay_path=_required_string(payload, "python_overlay_path"),
            custom_node_revisions=_custom_node_revision_payload(payload),
            overlay_dependency_record_paths=_string_list(payload, "overlay_dependency_record_paths"),
            model_asset_paths=_string_list(payload, "model_asset_paths"),
            prepared_at=_required_string(payload, "prepared_at"),
        )
    except OSError as error:
        raise PreparationError("Prepared runtime manifest is missing.") from error
    except (json.JSONDecodeError, TypeError, ValueError) as error:
        raise PreparationError("Prepared runtime manifest is invalid.") from error


def validate_manifest(manifest: PreparedRuntimeManifest, *, paths: RuntimePaths) -> None:
    if manifest.environment_kind != ENVIRONMENT_KIND:
        raise PreparationError("Prepared runtime environment kind is invalid.")
    if Path(manifest.python_path).resolve(strict=False) != paths.image_python_path.resolve(strict=False):
        raise PreparationError("Prepared runtime Python path is invalid.")
    if Path(manifest.comfyui_root).resolve(strict=False) != paths.image_comfyui_root.resolve(strict=False):
        raise PreparationError("Prepared runtime ComfyUI path is invalid.")
    if Path(manifest.workspace_root).resolve(strict=False) != paths.workspace_root.resolve(strict=False):
        raise PreparationError("Prepared runtime workspace path is invalid.")
    if Path(manifest.python_overlay_path).resolve(strict=False) != paths.python_overlay_path.resolve(strict=False):
        raise PreparationError("Prepared runtime overlay path is invalid.")


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
