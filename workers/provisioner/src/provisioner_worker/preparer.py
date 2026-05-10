from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from subprocess import DEVNULL, STDOUT, Popen, TimeoutExpired
from threading import Event
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import urlopen

from provisioner_worker.errors import PreparationError
from provisioner_worker.paths import safe_child_path, safe_custom_node_child_path
from provisioner_worker.schemas import CustomNode, GitSource, ModelAsset, StartRequest

ProgressCallback = Callable[[str, int | None, str | None], None]


@dataclass(frozen=True)
class CommandRunner:
    def run(self, args: list[str], *, cwd: Path | None = None, cancel_event: Event | None = None) -> None:
        process = Popen(args, cwd=cwd, stdout=DEVNULL, stderr=STDOUT, text=True)
        while process.poll() is None:
            if cancel_event is not None and cancel_event.is_set():
                process.terminate()
                try:
                    process.wait(timeout=5)
                except TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                raise Cancelled()
            try:
                process.wait(timeout=0.1)
            except TimeoutExpired:
                pass

        if process.returncode != 0:
            command = " ".join(args[:2])
            raise PreparationError(f"Command failed: {command}")


@dataclass(frozen=True)
class PublicFileDownloader:
    def download(self, url: str, target: Path, *, cancel_event: Event | None = None) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary = target.with_suffix(target.suffix + ".part")
        try:
            with urlopen(url, timeout=60) as response, temporary.open("wb") as output:
                while True:
                    if cancel_event is not None and cancel_event.is_set():
                        raise Cancelled()
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    output.write(chunk)
            temporary.replace(target)
        except HTTPError as error:
            if error.code in (401, 403):
                raise PreparationError("Hugging Face asset requires authentication") from error
            raise PreparationError("Hugging Face asset download failed") from error
        except (OSError, URLError) as error:
            raise PreparationError("Hugging Face asset download failed") from error
        finally:
            if temporary.exists():
                temporary.unlink()


class Provisioner:
    def __init__(
        self,
        *,
        command_runner: CommandRunner | None = None,
        downloader: PublicFileDownloader | None = None,
    ):
        self.command_runner = command_runner or CommandRunner()
        self.downloader = downloader or PublicFileDownloader()

    def prepare(self, request: StartRequest, progress: ProgressCallback, cancel_event: Event) -> None:
        workspace_root = request.workspace_mount_path.resolve(strict=False)
        workspace_root.mkdir(parents=True, exist_ok=True)
        comfyui_root = workspace_root / "ComfyUI"

        self._check_cancelled(cancel_event)
        progress("installing_comfyui", 5, "Preparing ComfyUI")
        self._checkout_git(request.workflow_preset.required_comfyui_source, comfyui_root, cancel_event)

        self._check_cancelled(cancel_event)
        progress("installing_comfyui", 25, "Installing ComfyUI dependencies")
        self._install_requirements(comfyui_root / "requirements.txt", cwd=comfyui_root, cancel_event=cancel_event)

        self._install_custom_nodes(request.workflow_preset.required_custom_nodes, comfyui_root, progress, cancel_event)
        self._download_assets(request.workflow_preset.required_model_assets, comfyui_root, progress, cancel_event)

        self._check_cancelled(cancel_event)
        progress("validating_environment", 95, "Validating prepared environment")
        self._validate_environment(request, comfyui_root)
        progress("validating_environment", 100, "Environment prepared")

    def _checkout_git(self, source: GitSource, target: Path, cancel_event: Event) -> None:
        if target.exists():
            self.command_runner.run(["git", "fetch", "--all", "--tags"], cwd=target, cancel_event=cancel_event)
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            self.command_runner.run(["git", "clone", source.repository_url, str(target)], cancel_event=cancel_event)

        self.command_runner.run(["git", "checkout", source.revision], cwd=target, cancel_event=cancel_event)

    def _install_requirements(self, requirements_path: Path, *, cwd: Path, cancel_event: Event) -> None:
        if requirements_path.exists():
            self.command_runner.run(
                ["python", "-m", "pip", "install", "-r", str(requirements_path)],
                cwd=cwd,
                cancel_event=cancel_event,
            )

    def _install_custom_nodes(
        self,
        custom_nodes: list[CustomNode],
        comfyui_root: Path,
        progress: ProgressCallback,
        cancel_event: Event,
    ) -> None:
        if not custom_nodes:
            return

        for index, node in enumerate(custom_nodes, start=1):
            self._check_cancelled(cancel_event)
            progress("installing_custom_nodes", 30 + index, f"Installing Custom Node {node.name}")
            target = safe_custom_node_child_path(
                comfyui_root,
                node.install.comfyui_custom_nodes_relative_path.as_posix(),
                field_name=f"custom_node[{node.id}].install.comfyui_custom_nodes_relative_path",
            )
            self._checkout_git(node.git_source, target, cancel_event)
            if node.install.python_requirements_path is not None:
                requirements_path = safe_child_path(
                    target,
                    node.install.python_requirements_path.as_posix(),
                    field_name=f"custom_node[{node.id}].install.python_requirements_path",
                )
                self._install_requirements(requirements_path, cwd=target, cancel_event=cancel_event)

    def _download_assets(
        self,
        assets: list[ModelAsset],
        comfyui_root: Path,
        progress: ProgressCallback,
        cancel_event: Event,
    ) -> None:
        for index, asset in enumerate(assets, start=1):
            self._check_cancelled(cancel_event)
            progress("downloading_assets", 55 + index, f"Downloading model asset {asset.name}")
            target = safe_child_path(
                comfyui_root,
                asset.install.comfyui_relative_path.as_posix(),
                field_name=f"model_asset[{asset.id}].install.comfyui_relative_path",
            )
            if target.exists() and (asset.file_size_bytes == 0 or target.stat().st_size == asset.file_size_bytes):
                continue
            self.downloader.download(huggingface_url(asset), target, cancel_event=cancel_event)
            if asset.file_size_bytes > 0 and target.stat().st_size != asset.file_size_bytes:
                raise PreparationError(f"Downloaded asset size mismatch: {asset.id}")

    def _validate_environment(self, request: StartRequest, comfyui_root: Path) -> None:
        if not comfyui_root.exists() or not comfyui_root.is_dir():
            raise PreparationError("ComfyUI directory is missing")
        if not (comfyui_root / "main.py").is_file():
            raise PreparationError("ComfyUI entrypoint is missing")

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

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()


class Cancelled(Exception):
    pass


def huggingface_url(asset: ModelAsset) -> str:
    source = asset.download_source
    return (
        "https://huggingface.co/"
        f"{quote(source.repository_id, safe='/')}/resolve/"
        f"{quote(source.revision, safe='')}/"
        f"{quote(source.file_path, safe='/')}"
    )
