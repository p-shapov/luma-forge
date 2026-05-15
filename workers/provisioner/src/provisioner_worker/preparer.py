from collections.abc import Callable
from dataclasses import dataclass
import hashlib
import json
from multiprocessing import get_context
from pathlib import Path
from queue import Empty
import string
from subprocess import DEVNULL, PIPE, STDOUT, Popen, TimeoutExpired
from threading import Event
from time import monotonic

from provisioner_worker.config import WorkerConfig
from provisioner_worker.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    DependencyInstallError,
    GitCheckoutError,
    PreparationError,
    StepTimeoutError,
    WorkerError,
)
from provisioner_worker.paths import safe_child_path, safe_custom_node_child_path
from provisioner_worker.runtime import (
    build_manifest,
    load_manifest,
    runtime_paths,
    validate_manifest,
    write_manifest,
)
from provisioner_worker.schemas import CustomNode, GitSource, HuggingFaceSource, ModelAsset, StartRequest

ProgressCallback = Callable[[str, int | None, str | None], None]
HubDownload = Callable[..., str]


@dataclass(frozen=True)
class CommandRunner:
    def run(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
        error_type: type[PreparationError] = PreparationError,
    ) -> None:
        try:
            process = Popen(args, cwd=cwd, stdout=DEVNULL, stderr=STDOUT, text=True)
        except OSError as error:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}") from error

        deadline = None if timeout_seconds is None else monotonic() + timeout_seconds
        while process.poll() is None:
            if cancel_event is not None and cancel_event.is_set():
                process.terminate()
                try:
                    process.wait(timeout=5)
                except TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                raise Cancelled()
            if deadline is not None and monotonic() >= deadline:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                raise StepTimeoutError("Provisioning step timed out.")
            try:
                process.wait(timeout=0.1)
            except TimeoutExpired:
                pass

        if process.returncode != 0:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}")

    def capture(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
        error_type: type[PreparationError] = PreparationError,
    ) -> str:
        try:
            process = Popen(args, cwd=cwd, stdout=PIPE, stderr=STDOUT, text=True)
        except OSError as error:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}") from error

        deadline = None if timeout_seconds is None else monotonic() + timeout_seconds
        while True:
            if cancel_event is not None and cancel_event.is_set():
                process.terminate()
                try:
                    process.communicate(timeout=5)
                except TimeoutExpired:
                    process.kill()
                    process.communicate(timeout=5)
                raise Cancelled()
            if deadline is not None and monotonic() >= deadline:
                process.terminate()
                try:
                    process.communicate(timeout=5)
                except TimeoutExpired:
                    process.kill()
                    process.communicate(timeout=5)
                raise StepTimeoutError("Provisioning step timed out.")
            communicate_timeout = 0.1
            if deadline is not None:
                communicate_timeout = min(communicate_timeout, max(0.0, deadline - monotonic()))
            try:
                output, _ = process.communicate(timeout=communicate_timeout)
                break
            except TimeoutExpired:
                continue

        if process.returncode != 0:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}")
        return output


@dataclass(frozen=True)
class PublicFileDownloader:
    hub_download: HubDownload | None = None

    def download(
        self,
        asset: ModelAsset,
        target: Path,
        *,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
    ) -> None:
        target.parent.mkdir(parents=True, exist_ok=True)
        source = asset.download_source
        try:
            cached_path = _download_with_isolated_process(
                source,
                target.parent,
                self.hub_download,
                timeout_seconds=timeout_seconds,
                cancel_event=cancel_event,
            )
            self._place_downloaded_file(Path(cached_path), target, cancel_event=cancel_event)
        except Cancelled:
            raise
        except WorkerError:
            raise
        except Exception as error:
            if _is_huggingface_auth_error(error):
                raise AssetAuthRequiredError("Hugging Face asset requires authentication.") from error
            raise AssetDownloadError("Hugging Face asset download failed.") from error

    def _place_downloaded_file(self, downloaded_path: Path, target: Path, *, cancel_event: Event | None) -> None:
        if downloaded_path.resolve(strict=False) == target.resolve(strict=False):
            return

        temporary = target.with_suffix(target.suffix + ".part")
        try:
            with downloaded_path.open("rb") as source, temporary.open("wb") as output:
                while True:
                    if cancel_event is not None and cancel_event.is_set():
                        raise Cancelled()
                    chunk = source.read(1024 * 1024)
                    if not chunk:
                        break
                    output.write(chunk)
            temporary.replace(target)
        finally:
            if temporary.exists():
                temporary.unlink()


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

    def prepare(self, request: StartRequest, progress: ProgressCallback, cancel_event: Event) -> None:
        workspace_root = self.config.workspace_mount_path.resolve(strict=False)
        workspace_root.mkdir(parents=True, exist_ok=True)
        paths = runtime_paths(workspace_root)
        comfyui_root = paths.comfyui_root

        self._check_cancelled(cancel_event)
        progress("installing_comfyui", 5, "Preparing ComfyUI")
        self._checkout_git(request.workflow_preset.required_comfyui_source, comfyui_root, cancel_event)

        self._check_cancelled(cancel_event)
        progress("installing_comfyui", 20, "Preparing volume Python environment")
        self._ensure_volume_venv(paths.venv_dir, cancel_event)

        self._check_cancelled(cancel_event)
        progress("installing_comfyui", 25, "Installing ComfyUI dependencies into volume environment")
        self._install_requirements(
            comfyui_root / "requirements.txt",
            cwd=comfyui_root,
            python_path=paths.python_path,
            report_label="comfyui",
            metadata_dir=paths.metadata_dir,
            cancel_event=cancel_event,
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
        python_version = self._capture_python_version(paths.python_path, cancel_event)
        self._write_dependency_records(paths, cancel_event)
        self._validate_environment(request, paths, include_manifest=False)
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
        self._validate_environment(request, paths, include_manifest=True)
        progress("validating_environment", 100, "Environment prepared")

    def _checkout_git(self, source: GitSource, target: Path, cancel_event: Event) -> None:
        if target.exists():
            self.command_runner.run(
                ["git", "fetch", "--all", "--tags"],
                cwd=target,
                cancel_event=cancel_event,
                timeout_seconds=self.config.git_timeout_seconds,
                error_type=GitCheckoutError,
            )
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            self.command_runner.run(
                ["git", "clone", source.repository_url, str(target)],
                cancel_event=cancel_event,
                timeout_seconds=self.config.git_timeout_seconds,
                error_type=GitCheckoutError,
            )

        self.command_runner.run(
            ["git", "checkout", source.revision],
            cwd=target,
            cancel_event=cancel_event,
            timeout_seconds=self.config.git_timeout_seconds,
            error_type=GitCheckoutError,
        )

    def _ensure_volume_venv(self, venv_dir: Path, cancel_event: Event) -> None:
        if (venv_dir / "bin" / "python").is_file():
            return
        venv_dir.parent.mkdir(parents=True, exist_ok=True)
        self.command_runner.run(
            ["python", "-m", "venv", str(venv_dir)],
            cancel_event=cancel_event,
            timeout_seconds=self.config.dependency_timeout_seconds,
            error_type=DependencyInstallError,
        )

    def _install_requirements(
        self,
        requirements_path: Path,
        *,
        cwd: Path,
        python_path: Path,
        report_label: str,
        metadata_dir: Path,
        cancel_event: Event,
    ) -> None:
        if requirements_path.exists():
            metadata_dir.mkdir(parents=True, exist_ok=True)
            report_path = _metadata_report_path(metadata_dir, report_label)
            self.command_runner.run(
                [
                    str(python_path),
                    "-m",
                    "pip",
                    "install",
                    "--report",
                    str(report_path),
                    "-r",
                    str(requirements_path),
                ],
                cwd=cwd,
                cancel_event=cancel_event,
                timeout_seconds=self.config.dependency_timeout_seconds,
                error_type=DependencyInstallError,
            )

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
                self._install_requirements(
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
        for index, asset in enumerate(assets, start=1):
            self._check_cancelled(cancel_event)
            progress("downloading_assets", 55 + index, f"Downloading model asset {asset.name}")
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

    def _write_dependency_records(self, paths, cancel_event: Event) -> None:
        paths.metadata_dir.mkdir(parents=True, exist_ok=True)
        freeze = self.command_runner.capture(
            [str(paths.python_path), "-m", "pip", "freeze"],
            cancel_event=cancel_event,
            timeout_seconds=self.config.dependency_timeout_seconds,
            error_type=DependencyInstallError,
        )
        paths.pip_freeze_path.write_text(freeze, encoding="utf-8")
        install_reports = sorted(path.name for path in paths.metadata_dir.glob("*-install-report.json"))
        paths.install_report_path.write_text(
            json.dumps({"reports": install_reports}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def _capture_python_version(self, python_path: Path, cancel_event: Event) -> str:
        return self.command_runner.capture(
            [str(python_path), "--version"],
            cancel_event=cancel_event,
            timeout_seconds=self.config.dependency_timeout_seconds,
            error_type=DependencyInstallError,
        ).strip()

    def _validate_environment(self, request: StartRequest, paths, *, include_manifest: bool) -> None:
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

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()


class Cancelled(Exception):
    pass


def _load_hub_download() -> HubDownload:
    try:
        from huggingface_hub import hf_hub_download
    except ImportError as error:
        raise AssetDownloadError("Hugging Face Hub client is unavailable.") from error
    return hf_hub_download


def _download_with_isolated_process(
    source: HuggingFaceSource,
    local_dir: Path,
    hub_download: HubDownload | None,
    *,
    timeout_seconds: float | None,
    cancel_event: Event | None,
) -> str:
    if timeout_seconds is None:
        return _download_from_hub(source, local_dir, hub_download)

    context = get_context("spawn")
    result_queue = context.Queue()
    process = context.Process(
        target=_hub_download_process,
        args=(source, str(local_dir), hub_download, result_queue),
    )
    process.start()

    deadline = None if timeout_seconds is None else monotonic() + timeout_seconds
    while process.is_alive():
        if cancel_event is not None and cancel_event.is_set():
            _terminate_process(process)
            raise Cancelled()
        if deadline is not None and monotonic() >= deadline:
            _terminate_process(process)
            raise StepTimeoutError("Hugging Face asset download timed out.")
        process.join(timeout=0.1)

    try:
        status, *payload = result_queue.get(timeout=1)
    except Empty as error:
        raise AssetDownloadError("Hugging Face asset download failed.") from error

    if status == "ok":
        return payload[0]

    error_class, status_code, message = payload
    if error_class == "GatedRepoError" or status_code in (401, 403):
        raise AssetAuthRequiredError("Hugging Face asset requires authentication.")
    raise AssetDownloadError("Hugging Face asset download failed.") from RuntimeError(message)


def _download_from_hub(source: HuggingFaceSource, local_dir: Path, hub_download: HubDownload | None) -> str:
    download = hub_download or _load_hub_download()
    return download(
        repo_id=source.repository_id,
        filename=source.file_path,
        revision=source.revision,
        repo_type="model",
        local_dir=str(local_dir),
        token=False,
    )


def _hub_download_process(
    source: HuggingFaceSource,
    local_dir: str,
    hub_download: HubDownload | None,
    result_queue,
) -> None:
    try:
        result_queue.put(("ok", _download_from_hub(source, Path(local_dir), hub_download)))
    except BaseException as error:
        response = getattr(error, "response", None)
        status_code = getattr(response, "status_code", None)
        result_queue.put(("error", error.__class__.__name__, status_code, str(error)))


def _terminate_process(process) -> None:
    process.terminate()
    process.join(timeout=5)
    if process.is_alive():
        process.kill()
        process.join(timeout=5)


def _metadata_report_path(metadata_dir: Path, report_label: str) -> Path:
    safe_chars = string.ascii_letters + string.digits + "._-"
    safe_label = "".join(character if character in safe_chars else "-" for character in report_label).strip(".-_")
    if safe_label == "":
        raise DependencyInstallError("Dependency install report label is invalid.")
    if safe_label != report_label:
        digest = hashlib.sha256(report_label.encode("utf-8")).hexdigest()[:12]
        safe_label = f"{safe_label}-{digest}"

    metadata_root = metadata_dir.resolve(strict=False)
    report_path = (metadata_root / f"{safe_label}-install-report.json").resolve(strict=False)
    if report_path.parent != metadata_root:
        raise DependencyInstallError("Dependency install report path is invalid.")
    return report_path


def _is_huggingface_auth_error(error: Exception) -> bool:
    if error.__class__.__name__ == "GatedRepoError":
        return True
    response = getattr(error, "response", None)
    status_code = getattr(response, "status_code", None)
    return status_code in (401, 403)
