import tempfile
import tarfile
import unittest
from pathlib import Path
from threading import Event

from app.errors import PreparationError
from app.schemas import parse_start_request
from helpers import start_payload, test_config
from runtime.manifest import runtime_paths
from runtime.materializer import RuntimeMaterializer


class MaterializerTests(unittest.TestCase):
    def test_rejects_mismatched_runtime_contract_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory), runtime_contract_id="other-runtime")

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory)),
                    Event(),
                )

    def test_rejects_mismatched_provisioner_image_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(
                workspace_mount_path=Path(directory),
                provisioner_image_ref="ghcr.io/luma-forge/provisioner-worker@sha256:9999999999999999999999999999999999999999999999999999999999999999",
            )

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory)),
                    Event(),
                )

    def test_materializes_upstream_comfyui_custom_nodes_from_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))

            RuntimeMaterializer(test_config(workspace_mount_path=Path(directory))).materialize(
                request.resolved_runtime_implementation,
                paths,
                Event(),
            )

            self.assertTrue((paths.comfyui_root / "custom_nodes" / "websocket_image_save.py").is_file())
            self.assertTrue((paths.metadata_dir / "base-runtime/pip-freeze.txt").is_file())

    def test_rejects_archive_missing_declared_base_dependency_record(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            archive_path = _runtime_archive_without_base_records()
            config = test_config(workspace_mount_path=Path(directory), runtime_archive_path=archive_path)

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory)),
                    Event(),
                )

    def test_reads_gzip_runtime_archive_format(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            archive_path = _runtime_archive_with_base_records()
            paths = runtime_paths(Path(directory))
            config = test_config(workspace_mount_path=Path(directory), runtime_archive_path=archive_path)

            RuntimeMaterializer(config).materialize(
                request.resolved_runtime_implementation,
                paths,
                Event(),
            )

            self.assertTrue((paths.metadata_dir / "base-runtime/pip-freeze.txt").is_file())

def _runtime_archive_with_base_records() -> Path:
    return _runtime_archive(include_base_records=True)


def _runtime_archive_without_base_records() -> Path:
    return _runtime_archive(include_base_records=False)


def _runtime_archive(*, include_base_records: bool) -> Path:
    archive_file = tempfile.NamedTemporaryFile(prefix="luma-forge-runtime-test-", suffix=".tar.gz", delete=False)
    archive_path = Path(archive_file.name)
    archive_file.close()
    with tempfile.TemporaryDirectory() as source_directory:
        root = Path(source_directory)
        comfyui = root / "ComfyUI"
        custom_nodes = comfyui / "custom_nodes"
        venv_bin = root / ".venv/bin"
        custom_nodes.mkdir(parents=True)
        venv_bin.mkdir(parents=True)
        (comfyui / "main.py").write_text("# ComfyUI\n", encoding="utf-8")
        (custom_nodes / "websocket_image_save.py").write_text("# node\n", encoding="utf-8")
        (venv_bin / "python").write_text("#!/usr/bin/env python\n", encoding="utf-8")
        with tarfile.open(archive_path, "w:gz") as archive:
            archive.add(comfyui, arcname="ComfyUI")
            archive.add(root / ".venv", arcname=".venv")
            if include_base_records:
                base_runtime = root / ".luma-forge/base-runtime"
                base_runtime.mkdir(parents=True)
                (base_runtime / "pip-freeze.txt").write_text("torch==2.5.1\n", encoding="utf-8")
                archive.add(base_runtime, arcname=".luma-forge/base-runtime")
    return archive_path


if __name__ == "__main__":
    unittest.main()
