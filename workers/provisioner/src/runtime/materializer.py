from collections.abc import Callable
import json
from pathlib import Path
import shutil
from threading import Event

from app.config import WorkerConfig
from app.errors import PreparationError
from app.schemas import ResolvedRuntimeImplementation
from auxiliary.command_runner import Cancelled
from runtime.manifest import RuntimePaths


ProgressCallback = Callable[[str, int | None, str | None], None]


class RuntimeMaterializer:
    def __init__(self, config: WorkerConfig):
        self.config = config

    def validate_request(self, resolved: ResolvedRuntimeImplementation) -> None:
        expected = {
            "contract_id": self.config.runtime_contract_id,
            "contract_version": self.config.runtime_contract_version,
            "implementation_revision": self.config.runtime_implementation_revision,
            "provisioner_image_ref": self.config.provisioner_image_ref,
        }
        actual = {
            "contract_id": resolved.contract_id,
            "contract_version": resolved.contract_version,
            "implementation_revision": resolved.implementation_revision,
            "provisioner_image_ref": resolved.provisioner_image_ref,
        }
        if actual != expected:
            raise PreparationError("Resolved runtime implementation does not match this Provisioner image.")

    def materialize(
        self,
        resolved: ResolvedRuntimeImplementation,
        paths: RuntimePaths,
        cancel_event: Event,
        progress: ProgressCallback | None = None,
    ) -> None:
        self.validate_request(resolved)
        _report(progress, 6, "Runtime implementation matches provisioner image")
        self._check_cancelled(cancel_event)
        self._validate_image_runtime(resolved, paths)
        _report(progress, 14, "Image-baked runtime validated")
        self._prepare_workspace_paths(paths, cancel_event)
        _report(progress, 25, "Workspace runtime paths prepared")
        self._validate_dependency_records(resolved, paths)
        _report(progress, 30, "Base runtime records validated")

    def _validate_image_runtime(
        self,
        resolved: ResolvedRuntimeImplementation,
        paths: RuntimePaths,
    ) -> None:
        image_root = self.config.image_runtime_root_path.resolve(strict=False)
        if paths.image_runtime_root.resolve(strict=False) != image_root:
            raise PreparationError("Configured image runtime root is invalid.")
        if not paths.image_runtime_root.is_dir():
            raise PreparationError("Image-baked runtime root is missing.")
        self._validate_image_metadata(resolved, paths)
        if not paths.image_python_path.is_file():
            raise PreparationError("Image-baked Python interpreter is missing.")
        if not paths.image_comfyui_root.is_dir():
            raise PreparationError("Image-baked ComfyUI directory is missing.")
        if not (paths.image_comfyui_root / "main.py").is_file():
            raise PreparationError("Image-baked ComfyUI entrypoint is missing.")
        if resolved.image_metadata.image_base_dependency_record_paths == []:
            raise PreparationError("Image-baked base dependency records are missing.")

    def _validate_image_metadata(
        self,
        resolved: ResolvedRuntimeImplementation,
        paths: RuntimePaths,
    ) -> None:
        declared_root = Path(resolved.image_metadata.image_runtime_root_path).resolve(strict=False)
        declared_metadata_path = Path(resolved.image_metadata.provisioner_runtime_metadata_path).resolve(strict=False)
        if declared_metadata_path != declared_root and declared_root not in declared_metadata_path.parents:
            raise PreparationError("Image-baked runtime metadata path is invalid.")
        metadata_path = paths.image_runtime_root / declared_metadata_path.relative_to(declared_root)
        try:
            payload = json.loads(metadata_path.read_text(encoding="utf-8"))
        except OSError as error:
            raise PreparationError("Image-baked runtime metadata is missing.") from error
        except json.JSONDecodeError as error:
            raise PreparationError("Image-baked runtime metadata is invalid.") from error
        if not isinstance(payload, dict):
            raise PreparationError("Image-baked runtime metadata is invalid.")

        expected = {
            "contract_id": resolved.contract_id,
            "contract_version": resolved.contract_version,
            "implementation_revision": resolved.implementation_revision,
            "environment_kind": resolved.runtime_metadata.environment_kind,
            "python_version": resolved.runtime_metadata.python_version,
            "platform": resolved.runtime_metadata.platform,
            "comfyui_revision": resolved.runtime_metadata.comfyui_revision,
            "image_runtime_root_path": str(resolved.image_metadata.image_runtime_root_path),
            "image_python_interpreter_path": str(resolved.image_metadata.image_python_interpreter_path),
            "image_comfyui_root_path": str(resolved.image_metadata.image_comfyui_root_path),
            "image_base_dependency_record_paths": [
                path.as_posix()
                for path in resolved.image_metadata.image_base_dependency_record_paths
            ],
            "runtime_manifest_compatibility": resolved.runtime_metadata.runtime_manifest_compatibility,
            "workspace_overlay_policy": {
                "python_overlay_path": resolved.runtime_metadata.workspace_overlay_policy.python_overlay_path.as_posix(),
                "import_path_precedence": resolved.runtime_metadata.workspace_overlay_policy.import_path_precedence,
                "protected_package_names": resolved.runtime_metadata.workspace_overlay_policy.protected_package_names,
                "protected_package_prefixes": resolved.runtime_metadata.workspace_overlay_policy.protected_package_prefixes,
            },
        }
        for key, expected_value in expected.items():
            if payload.get(key) != expected_value:
                raise PreparationError("Image-baked runtime metadata does not match resolved implementation.")

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

    def _validate_dependency_records(self, resolved: ResolvedRuntimeImplementation, paths: RuntimePaths) -> None:
        image_root = paths.image_runtime_root.resolve(strict=False)
        for record_path in resolved.image_metadata.image_base_dependency_record_paths:
            target = (image_root / record_path).resolve(strict=False)
            if target != image_root and image_root not in target.parents:
                raise PreparationError("Image-baked base dependency record path is invalid.")
            if not target.is_file():
                raise PreparationError("Image-baked base dependency record is missing.")

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()


def _report(progress: ProgressCallback | None, percent: int, message: str) -> None:
    if progress is not None:
        progress("materializing_runtime", percent, message)
