from collections.abc import Callable
import shutil
from threading import Event

from app.config import WorkerConfig
from app.errors import PreparationError
from app.schemas import ResolvedRuntimeImage
from auxiliary.command_runner import Cancelled
from runtime.manifest import RuntimePaths


ProgressCallback = Callable[[str, int | None, str | None], None]


class RuntimeMaterializer:
    def __init__(self, config: WorkerConfig):
        self.config = config

    def materialize(
        self,
        resolved: ResolvedRuntimeImage,
        paths: RuntimePaths,
        cancel_event: Event,
        progress: ProgressCallback | None = None,
    ) -> None:
        _ = resolved
        _report(progress, 6, "Runtime image snapshot accepted")
        self._check_cancelled(cancel_event)
        self._validate_image_runtime(paths)
        _report(progress, 14, "Image-baked runtime validated")
        self._prepare_workspace_paths(paths, cancel_event)
        _report(progress, 25, "Workspace runtime paths prepared")

    def _validate_image_runtime(
        self,
        paths: RuntimePaths,
    ) -> None:
        image_root = self.config.image_runtime_root_path.resolve(strict=False)
        if paths.image_runtime_root.resolve(strict=False) != image_root:
            raise PreparationError("Configured image runtime root is invalid.")
        if not paths.image_runtime_root.is_dir():
            raise PreparationError("Image-baked runtime root is missing.")
        if not paths.image_python_path.is_file():
            raise PreparationError("Image-baked Python interpreter is missing.")
        if not paths.image_comfyui_root.is_dir():
            raise PreparationError("Image-baked ComfyUI directory is missing.")
        if not (paths.image_comfyui_root / "main.py").is_file():
            raise PreparationError("Image-baked ComfyUI entrypoint is missing.")
    def _prepare_workspace_paths(self, paths: RuntimePaths, cancel_event: Event) -> None:
        for directory in [
            paths.workspace_root / "models",
            paths.workspace_root / "custom_nodes",
            paths.workspace_root / "output",
            paths.metadata_dir,
        ]:
            self._check_cancelled(cancel_event)
            directory.mkdir(parents=True, exist_ok=True)
        self._reset_python_overlay(paths, cancel_event)

    def _reset_python_overlay(self, paths: RuntimePaths, cancel_event: Event) -> None:
        self._check_cancelled(cancel_event)
        if paths.python_overlay_path.exists():
            if paths.python_overlay_path.is_symlink() or not paths.python_overlay_path.is_dir():
                raise PreparationError("Prepared Python overlay path is invalid.")
            shutil.rmtree(paths.python_overlay_path)
        paths.python_overlay_path.mkdir(parents=True, exist_ok=True)

        for report_path in paths.metadata_dir.glob("custom-node-*-install-report.json"):
            self._check_cancelled(cancel_event)
            if report_path.is_file():
                report_path.unlink()

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()


def _report(progress: ProgressCallback | None, percent: int, message: str) -> None:
    if progress is not None:
        progress("materializing_runtime", percent, message)
