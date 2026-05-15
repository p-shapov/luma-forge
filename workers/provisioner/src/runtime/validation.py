from app.errors import PreparationError
from auxiliary.paths import safe_child_path, safe_custom_node_child_path
from runtime.manifest import RuntimePaths, load_manifest, validate_manifest
from app.schemas import StartRequest


def validate_prepared_environment(request: StartRequest, paths: RuntimePaths, *, include_manifest: bool) -> None:
    comfyui_root = paths.comfyui_root
    if not comfyui_root.exists() or not comfyui_root.is_dir():
        raise PreparationError("ComfyUI directory is missing")
    if not (comfyui_root / "main.py").is_file():
        raise PreparationError("ComfyUI entrypoint is missing")
    if not paths.python_path.is_file():
        raise PreparationError("Volume Python interpreter is missing")
    if not paths.pip_freeze_path.is_file():
        raise PreparationError("Dependency freeze record is missing")
    if not paths.install_report_path.is_file():
        raise PreparationError("Dependency install report is missing")

    for node in request.workflow_preset.required_custom_nodes:
        target = safe_custom_node_child_path(
            comfyui_root,
            node.install.comfyui_custom_nodes_relative_path.as_posix(),
            field_name=f"custom_node[{node.id}].install.comfyui_custom_nodes_relative_path",
        )
        if not target.exists() or not target.is_dir():
            raise PreparationError(f"Custom Node is missing: {node.id}")

    for asset in request.workflow_preset.required_model_assets:
        target = safe_child_path(
            comfyui_root,
            asset.install.comfyui_relative_path.as_posix(),
            field_name=f"model_asset[{asset.id}].install.comfyui_relative_path",
        )
        if not target.exists() or not target.is_file():
            raise PreparationError(f"Model asset is missing: {asset.id}")

    if include_manifest:
        manifest = load_manifest(paths.runtime_manifest_path)
        validate_manifest(manifest, paths=paths)
