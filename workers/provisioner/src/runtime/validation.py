from app.errors import PreparationError
from auxiliary.paths import safe_child_path
from runtime.manifest import RuntimePaths, load_manifest, validate_manifest
from app.schemas import StartRequest


def validate_prepared_environment(request: StartRequest, paths: RuntimePaths, *, include_manifest: bool) -> None:
    for asset in request.workflow_preset.required_model_assets:
        target = safe_child_path(
            paths.workspace_root,
            asset.install.comfyui_relative_path.as_posix(),
            field_name=f"model_asset[{asset.id}].install.comfyui_relative_path",
        )
        if not target.exists() or not target.is_file():
            raise PreparationError(f"Model asset is missing: {asset.id}")

    if include_manifest:
        manifest = load_manifest(paths.runtime_manifest_path)
        validate_manifest(manifest, paths=paths)
