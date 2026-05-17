import json
import re

from collections.abc import Callable
from collections.abc import Iterable
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
        overlay_policy = request.resolved_runtime_implementation.runtime_metadata.workspace_overlay_policy
        paths = runtime_paths(
            workspace_root,
            self.config.image_runtime_root_path,
            overlay_policy.python_overlay_path,
            request.resolved_runtime_implementation.image_metadata.image_python_interpreter_path,
            request.resolved_runtime_implementation.image_metadata.image_comfyui_root_path,
            request.resolved_runtime_implementation.image_metadata.image_runtime_root_path,
        )
        comfyui_root = paths.comfyui_root

        self._check_cancelled(cancel_event)
        progress("materializing_runtime", 5, "Validating image-baked ComfyUI runtime")
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
            paths.python_overlay_path,
            overlay_policy.protected_package_names,
            overlay_policy.protected_package_prefixes,
            progress,
            cancel_event,
        )
        self._download_assets(request.workflow_preset.required_model_assets, comfyui_root, progress, cancel_event)

        self._check_cancelled(cancel_event)
        progress("validating_environment", 90, "Recording prepared runtime environment")
        python_version = self.python_environment.capture_python_version(paths.image_python_path, cancel_event)
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
        overlay_path: Path,
        protected_package_names: list[str],
        protected_package_prefixes: list[str],
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
                _reject_protected_requirements(
                    requirements_path,
                    target,
                    protected_package_names,
                    protected_package_prefixes,
                )
                report_path = self.python_environment.install_requirements(
                    requirements_path,
                    cwd=target,
                    python_path=python_path,
                    target_path=overlay_path,
                    report_label=f"custom-node-{node.id}",
                    metadata_dir=metadata_dir,
                    cancel_event=cancel_event,
                )
                if report_path is not None:
                    _reject_protected_install_report(
                        report_path,
                        protected_package_names,
                        protected_package_prefixes,
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


def _reject_protected_requirements(
    requirements_path: Path,
    checkout_root: Path,
    protected_package_names: list[str],
    protected_package_prefixes: list[str],
) -> None:
    protected_names = {_normalize_package_name(name) for name in protected_package_names}
    protected_prefixes = [_normalize_package_name(prefix) for prefix in protected_package_prefixes]
    _reject_protected_requirements_file(
        requirements_path,
        checkout_root.resolve(strict=False),
        protected_names,
        protected_prefixes,
        set(),
    )


def _reject_protected_install_report(
    report_path: Path,
    protected_package_names: list[str],
    protected_package_prefixes: list[str],
) -> None:
    protected_names = {_normalize_package_name(name) for name in protected_package_names}
    protected_prefixes = [_normalize_package_name(prefix) for prefix in protected_package_prefixes]
    for name in _install_report_package_names(report_path):
        normalized = _normalize_package_name(name)
        if normalized in protected_names or any(normalized.startswith(prefix) for prefix in protected_prefixes):
            from app.errors import DependencyInstallError

            raise DependencyInstallError("Custom Node dependency conflicts with the image-baked base runtime.")


def _install_report_package_names(report_path: Path) -> Iterable[str]:
    from app.errors import DependencyInstallError

    try:
        payload = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise DependencyInstallError("Custom Node dependency install report is invalid.") from error

    installs = payload.get("install")
    if not isinstance(installs, list):
        raise DependencyInstallError("Custom Node dependency install report is invalid.")

    for entry in installs:
        if not isinstance(entry, dict):
            raise DependencyInstallError("Custom Node dependency install report is invalid.")
        metadata = entry.get("metadata")
        if not isinstance(metadata, dict):
            raise DependencyInstallError("Custom Node dependency install report is invalid.")
        name = metadata.get("name")
        if not isinstance(name, str) or name.strip() == "":
            raise DependencyInstallError("Custom Node dependency install report is invalid.")
        yield name


def _reject_protected_requirements_file(
    requirements_path: Path,
    root: Path,
    protected_names: set[str],
    protected_prefixes: list[str],
    visited: set[Path],
) -> None:
    if not requirements_path.exists():
        return
    resolved_path = requirements_path.resolve(strict=False)
    if resolved_path in visited:
        return
    if resolved_path != root and root not in resolved_path.parents:
        from app.errors import DependencyInstallError

        raise DependencyInstallError("Custom Node dependency requirements include an unsafe nested file.")
    visited.add(resolved_path)
    for raw_line in requirements_path.read_text(encoding="utf-8").splitlines():
        nested = _nested_requirements_path(raw_line, resolved_path.parent)
        if nested is not None:
            _reject_protected_requirements_file(
                nested,
                root,
                protected_names,
                protected_prefixes,
                visited,
            )
            continue
        name = _requirement_package_name(raw_line)
        if name is None:
            continue
        normalized = _normalize_package_name(name)
        if normalized in protected_names or any(normalized.startswith(prefix) for prefix in protected_prefixes):
            from app.errors import DependencyInstallError

            raise DependencyInstallError("Custom Node dependency conflicts with the image-baked base runtime.")


def _nested_requirements_path(line: str, current_dir: Path) -> Path | None:
    stripped = line.split("#", maxsplit=1)[0].strip()
    if stripped == "":
        return None
    parts = stripped.split()
    if len(parts) == 2 and parts[0] in ("-r", "--requirement", "-c", "--constraint"):
        if parts[1].startswith(("http://", "https://")):
            from app.errors import DependencyInstallError

            raise DependencyInstallError("Custom Node dependency requirements include an unreadable nested file.")
        return (current_dir / parts[1]).resolve(strict=False)
    for prefix in ("-r", "--requirement=", "-c", "--constraint="):
        if stripped.startswith(prefix):
            value = stripped[len(prefix):].strip()
            if value.startswith(("http://", "https://")):
                from app.errors import DependencyInstallError

                raise DependencyInstallError("Custom Node dependency requirements include an unreadable nested file.")
            if value:
                return (current_dir / value).resolve(strict=False)
    return None


def _requirement_package_name(line: str) -> str | None:
    egg_name = _direct_url_egg_package_name(line)
    if egg_name is not None:
        return egg_name

    stripped = line.split("#", maxsplit=1)[0].strip()
    if stripped == "" or stripped.startswith(("-", "http://", "https://", "git+")):
        return None
    for separator in ("==", ">=", "<=", "~=", "!=", ">", "<", "[", ";", " @ "):
        if separator in stripped:
            return stripped.split(separator, maxsplit=1)[0].strip()
    return stripped.strip()


def _direct_url_egg_package_name(line: str) -> str | None:
    marker = "#egg="
    if marker not in line:
        return None
    prefix = line.split(marker, maxsplit=1)[0].strip()
    if prefix == "" or prefix.startswith("#"):
        return None
    if not (
        "://" in prefix
        or prefix.startswith(("git+", "hg+", "svn+", "bzr+"))
        or prefix.startswith(("-e ", "--editable "))
    ):
        return None
    value = line.split(marker, maxsplit=1)[1]
    value = value.split("&", maxsplit=1)[0]
    value = value.split(";", maxsplit=1)[0]
    value = value.split("[", maxsplit=1)[0]
    value = value.strip()
    return value or None


def _normalize_package_name(value: str) -> str:
    return re.sub(r"[-_.]+", "-", value.strip().lower())
