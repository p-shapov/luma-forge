from collections.abc import Callable
from pathlib import Path
from threading import Event

from app.config import WorkerConfig
from app.errors import PreparationError
from app.schemas import ModelAsset, StartRequest
from auxiliary.cancellation import Cancelled
from auxiliary.huggingface import PublicFileDownloader
from auxiliary.paths import safe_child_path

ProgressCallback = Callable[[str, int | None, str | None], None]


def _phase_progress(start: int, end: int, index: int, total: int) -> int:
    if total <= 1:
        progress = start
    else:
        progress = start + ((index - 1) * (end - start)) // (total - 1)
    return max(0, min(100, progress))


class Provisioner:
    def __init__(
        self,
        *,
        downloader: PublicFileDownloader | None = None,
        config: WorkerConfig,
    ):
        self.downloader = downloader or PublicFileDownloader()
        self.config = config

    def prepare(self, request: StartRequest, progress: ProgressCallback, cancel_event: Event) -> None:
        workspace_root = self.config.workspace_mount_path.resolve(strict=False)
        workspace_root.mkdir(parents=True, exist_ok=True)

        self._check_cancelled(cancel_event)
        progress("preparing_workspace", 15, "Preparing workspace directories")
        self._prepare_workspace_paths(workspace_root, cancel_event)
        progress("preparing_workspace", 25, "Workspace directories prepared")

        self._download_assets(request.workflow_preset.required_model_assets, workspace_root, progress, cancel_event)

        self._check_cancelled(cancel_event)
        progress("validating_environment", 95, "Validating prepared model assets")
        self._validate_model_assets(request, workspace_root)
        progress("validating_environment", 100, "Environment prepared")

    def _download_assets(
        self,
        assets: list[ModelAsset],
        workspace_root: Path,
        progress: ProgressCallback,
        cancel_event: Event,
    ) -> None:
        total_assets = len(assets)
        for index, asset in enumerate(assets, start=1):
            self._check_cancelled(cancel_event)
            progress(
                "downloading_assets",
                _phase_progress(55, 90, index, total_assets),
                f"Downloading model asset {asset.name}",
            )
            target = safe_child_path(
                workspace_root,
                asset.install.comfyui_relative_path.as_posix(),
                field_name=f"model_asset[{asset.id}].install.comfyui_relative_path",
            )
            self.downloader.download(
                asset,
                target,
                cancel_event=cancel_event,
                timeout_seconds=self.config.download_timeout_seconds,
            )

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()

    def _prepare_workspace_paths(self, workspace_root: Path, cancel_event: Event) -> None:
        for directory in [
            workspace_root / "models",
            workspace_root / "workflows",
            workspace_root / "output",
        ]:
            self._check_cancelled(cancel_event)
            directory.mkdir(parents=True, exist_ok=True)

    def _validate_model_assets(self, request: StartRequest, workspace_root: Path) -> None:
        for asset in request.workflow_preset.required_model_assets:
            target = safe_child_path(
                workspace_root,
                asset.install.comfyui_relative_path.as_posix(),
                field_name=f"model_asset[{asset.id}].install.comfyui_relative_path",
            )
            if not target.exists() or not target.is_file():
                raise PreparationError(f"Model asset is missing: {asset.id}")
