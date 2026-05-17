import tempfile
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
                    runtime_paths(Path(directory), config.image_runtime_root_path),
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
                    runtime_paths(Path(directory), config.image_runtime_root_path),
                    Event(),
                )

    def test_validates_image_runtime_and_prepares_workspace_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            paths = runtime_paths(Path(directory), config.image_runtime_root_path)

            RuntimeMaterializer(config).materialize(
                request.resolved_runtime_implementation,
                paths,
                Event(),
            )

            self.assertTrue((paths.image_comfyui_root / "custom_nodes" / "websocket_image_save.py").is_file())
            self.assertTrue((paths.image_runtime_root / "base-runtime/pip-freeze.txt").is_file())
            self.assertTrue((Path(directory) / "models").is_dir())
            self.assertTrue((Path(directory) / "custom_nodes").is_dir())
            self.assertTrue((Path(directory) / "output").is_dir())
            self.assertTrue((Path(directory) / ".luma-forge/python-overlay").is_dir())
            self.assertFalse((Path(directory) / ".venv").exists())
            self.assertFalse((Path(directory) / "ComfyUI").exists())

    def test_resets_stale_python_overlay_and_install_reports(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            paths = runtime_paths(Path(directory), config.image_runtime_root_path)
            stale_package = paths.python_overlay_path / "stale_package.py"
            stale_package.parent.mkdir(parents=True)
            stale_package.write_text("stale = True\n", encoding="utf-8")
            stale_report = paths.metadata_dir / "custom-node-stale-install-report.json"
            stale_report.parent.mkdir(parents=True, exist_ok=True)
            stale_report.write_text('{"install":["stale"]}\n', encoding="utf-8")

            RuntimeMaterializer(config).materialize(
                request.resolved_runtime_implementation,
                paths,
                Event(),
            )

            self.assertTrue(paths.python_overlay_path.is_dir())
            self.assertFalse(stale_package.exists())
            self.assertFalse(stale_report.exists())

    def test_rejects_missing_declared_base_dependency_record(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            (config.image_runtime_root_path / "base-runtime/pip-freeze.txt").unlink()

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory), config.image_runtime_root_path),
                    Event(),
                )

    def test_rejects_mismatched_image_runtime_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            metadata_path = config.image_runtime_root_path / "runtime-metadata.json"
            payload = metadata_path.read_text(encoding="utf-8")
            metadata_path.write_text(
                payload.replace('"implementation_revision": "2026.05.16-001"', '"implementation_revision": "wrong"'),
                encoding="utf-8",
            )

            with self.assertRaises(PreparationError):
                RuntimeMaterializer(config).materialize(
                    request.resolved_runtime_implementation,
                    runtime_paths(Path(directory), config.image_runtime_root_path),
                    Event(),
                )


if __name__ == "__main__":
    unittest.main()
