from dataclasses import asdict, dataclass
from datetime import UTC, datetime
import json
from pathlib import Path

from app.errors import PreparationError
from app.schemas import StartRequest
from auxiliary.paths import safe_child_path

MANIFEST_KIND = "luma_forge_prepared_workspace"
METADATA_DIR = ".luma-forge"
RUNTIME_MANIFEST = "runtime-manifest.json"


@dataclass(frozen=True)
class RuntimePaths:
    workspace_root: Path
    metadata_dir: Path
    runtime_manifest_path: Path


@dataclass(frozen=True)
class PreparedRuntimeManifest:
    manifest_kind: str
    workspace_root: str
    model_asset_paths: list[str]
    prepared_at: str

    def to_json(self) -> str:
        return json.dumps(asdict(self), indent=2, sort_keys=True) + "\n"


def runtime_paths(
    workspace_root: Path,
) -> RuntimePaths:
    workspace = workspace_root.resolve(strict=False)
    metadata_dir = safe_child_path(workspace, METADATA_DIR, field_name="runtime_metadata_path")
    return RuntimePaths(
        workspace_root=workspace,
        metadata_dir=metadata_dir,
        runtime_manifest_path=metadata_dir / RUNTIME_MANIFEST,
    )


def build_manifest(
    *,
    request: StartRequest,
    paths: RuntimePaths,
) -> PreparedRuntimeManifest:
    return PreparedRuntimeManifest(
        manifest_kind=MANIFEST_KIND,
        workspace_root=str(paths.workspace_root),
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
            manifest_kind=_required_string(payload, "manifest_kind"),
            workspace_root=_required_string(payload, "workspace_root"),
            model_asset_paths=_string_list(payload, "model_asset_paths"),
            prepared_at=_required_string(payload, "prepared_at"),
        )
    except OSError as error:
        raise PreparationError("Prepared runtime manifest is missing.") from error
    except (json.JSONDecodeError, TypeError, ValueError) as error:
        raise PreparationError("Prepared runtime manifest is invalid.") from error


def validate_manifest(manifest: PreparedRuntimeManifest, *, paths: RuntimePaths) -> None:
    if manifest.manifest_kind != MANIFEST_KIND:
        raise PreparationError("Prepared workspace manifest kind is invalid.")
    if Path(manifest.workspace_root).resolve(strict=False) != paths.workspace_root.resolve(strict=False):
        raise PreparationError("Prepared workspace path is invalid.")


def _required_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or value.strip() == "":
        raise ValueError(f"{key} is required")
    return value


def _string_list(payload: dict[str, object], key: str) -> list[str]:
    value = payload.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) and item.strip() for item in value):
        raise ValueError(f"{key} is required")
    return value
