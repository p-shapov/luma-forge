from collections.abc import Callable
from pathlib import Path
from threading import Event

from auxiliary.command_runner import Cancelled, CommandRunner
from app.config import WorkerConfig
from auxiliary.huggingface import PublicFileDownloader
from runtime.validation import validate_prepared_environment
from auxiliary.git import GitCheckout
from auxiliary.paths import safe_child_path, safe_custom_node_child_path
from runtime.python_environment import PythonEnvironment
from runtime.manifest import build_manifest, runtime_paths, write_manifest
from runtime.materializer import RuntimeMaterializer
from app.schemas import CustomNode, ModelAsset, StartRequest

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
        command_runner: CommandRunner | None = None,
        downloader: PublicFileDownloader | None = None,
        config: WorkerConfig,
    ):
        self.command_runner = command_runner or CommandRunner()
        self.downloader = downloader or PublicFileDownloader()
        self.config = config
        self.git_checkout = GitCheckout(self.command_runner, self.config.git_timeout_seconds)
        self.python_environment = PythonEnvironment(
            self.command_runner,
            self.config.dependency_timeout_seconds,
        )
        self.runtime_materializer = RuntimeMaterializer(self.config)

    def prepare(self, request: StartRequest, progress: ProgressCallback, cancel_event: Event) -> None:
        workspace_root = self.config.workspace_mount_path.resolve(strict=False)
        workspace_root.mkdir(parents=True, exist_ok=True)
        paths = runtime_paths(workspace_root)
        comfyui_root = paths.comfyui_root

        self._check_cancelled(cancel_event)
        progress("materializing_runtime", 5, "Materializing image-baked ComfyUI runtime")
        self.runtime_materializer.materialize(
            request.resolved_runtime_implementation,
            paths,
            cancel_event,
            progress,
        )

        self._install_custom_nodes(
            request.workflow_preset.required_custom_nodes,
            comfyui_root,
            paths.python_path,
            paths.metadata_dir,
            progress,
            cancel_event,
        )
        self._download_assets(request.workflow_preset.required_model_assets, comfyui_root, progress, cancel_event)

        self._check_cancelled(cancel_event)
        progress("validating_environment", 90, "Recording prepared runtime environment")
        python_version = self.python_environment.capture_python_version(paths.python_path, cancel_event)
        validate_prepared_environment(request, paths, include_manifest=False)
        write_manifest(
            build_manifest(
                request=request,
                paths=paths,
                python_version=python_version,
            ),
            paths.runtime_manifest_path,
        )

        self._check_cancelled(cancel_event)
        progress("validating_environment", 95, "Validating prepared environment")
        validate_prepared_environment(request, paths, include_manifest=True)
        progress("validating_environment", 100, "Environment prepared")

    def _install_custom_nodes(
        self,
        custom_nodes: list[CustomNode],
        comfyui_root: Path,
        python_path: Path,
        metadata_dir: Path,
        progress: ProgressCallback,
        cancel_event: Event,
    ) -> None:
        if not custom_nodes:
            return

        total_nodes = len(custom_nodes)
        for index, node in enumerate(custom_nodes, start=1):
            self._check_cancelled(cancel_event)
            progress(
                "installing_custom_nodes",
                _phase_progress(30, 55, index, total_nodes),
                f"Installing Custom Node {node.id}",
            )
            target = safe_custom_node_child_path(
                comfyui_root,
                node.install.comfyui_custom_nodes_relative_path.as_posix(),
                field_name=f"custom_node[{node.id}].install.comfyui_custom_nodes_relative_path",
            )
            self.git_checkout.checkout(node.git_source, target, cancel_event)
            if node.install.python_requirements_path is not None:
                requirements_path = safe_child_path(
                    target,
                    node.install.python_requirements_path.as_posix(),
                    field_name=f"custom_node[{node.id}].install.python_requirements_path",
                )
                self.python_environment.install_requirements(
                    requirements_path,
                    cwd=target,
                    python_path=python_path,
                    report_label=f"custom-node-{node.id}",
                    metadata_dir=metadata_dir,
                    cancel_event=cancel_event,
                )

    def _download_assets(
        self,
        assets: list[ModelAsset],
        comfyui_root: Path,
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
                comfyui_root,
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
