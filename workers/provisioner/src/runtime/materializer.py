import shutil
import tarfile
from pathlib import Path
from threading import Event

from app.config import WorkerConfig
from app.errors import PreparationError
from app.schemas import ResolvedRuntimeImplementation
from auxiliary.command_runner import Cancelled
from runtime.manifest import RuntimePaths


STAGING_DIR = ".luma-forge/runtime-staging"


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

    def materialize(self, resolved: ResolvedRuntimeImplementation, paths: RuntimePaths, cancel_event: Event) -> None:
        self.validate_request(resolved)
        self._check_cancelled(cancel_event)
        archive_path = self.config.runtime_archive_path
        if not archive_path.is_file():
            archive_path = resolved.image_metadata.provisioner_runtime_archive_path
        if not archive_path.is_file():
            raise PreparationError("Image-baked runtime archive is missing.")

        staging_root = paths.workspace_root / STAGING_DIR
        if staging_root.exists():
            shutil.rmtree(staging_root)
        staging_root.mkdir(parents=True, exist_ok=True)

        try:
            with tarfile.open(archive_path, mode="r:*") as archive:
                archive.extractall(staging_root, filter="data")
        except (tarfile.TarError, OSError) as error:
            raise PreparationError("Image-baked runtime archive could not be extracted.") from error

        staged_comfyui = staging_root / "ComfyUI"
        staged_venv = staging_root / ".venv"
        staged_base_runtime = staging_root / ".luma-forge" / "base-runtime"
        self._validate_staged_runtime(staged_comfyui, staged_venv)
        self._publish(staged_comfyui, paths.comfyui_root, cancel_event)
        self._publish(staged_venv, paths.venv_dir, cancel_event)
        self._publish(staged_base_runtime, paths.metadata_dir / "base-runtime", cancel_event)
        self._validate_dependency_records(resolved, paths)

    def _validate_staged_runtime(self, staged_comfyui: Path, staged_venv: Path) -> None:
        if not (staged_comfyui / "main.py").is_file():
            raise PreparationError("Image-baked ComfyUI entrypoint is missing.")
        if not (staged_venv / "bin" / "python").is_file():
            raise PreparationError("Image-baked Python interpreter is missing.")
        if not (staged_comfyui.parent / ".luma-forge" / "base-runtime").is_dir():
            raise PreparationError("Image-baked base runtime records are missing.")

    def _validate_dependency_records(self, resolved: ResolvedRuntimeImplementation, paths: RuntimePaths) -> None:
        workspace = paths.workspace_root.resolve(strict=False)
        for record_path in resolved.runtime_metadata.base_dependency_record_paths:
            target = (workspace / record_path).resolve(strict=False)
            if target != workspace and workspace not in target.parents:
                raise PreparationError("Image-baked base dependency record path is invalid.")
            if not target.is_file():
                raise PreparationError("Image-baked base dependency record is missing.")

    def _publish(self, source: Path, target: Path, cancel_event: Event) -> None:
        self._check_cancelled(cancel_event)
        if target.exists():
            if target.is_dir():
                shutil.rmtree(target)
            else:
                target.unlink()
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(source), str(target))

    def _check_cancelled(self, cancel_event: Event) -> None:
        if cancel_event.is_set():
            raise Cancelled()
