import tempfile
import unittest
from pathlib import Path
from threading import Event

from app.schemas import parse_start_request
from helpers import start_payload, test_config
from runtime.manifest import runtime_paths
from runtime.materializer import RuntimeMaterializer


class MaterializerTests(unittest.TestCase):
    def test_validates_image_runtime_and_prepares_workspace_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            paths = runtime_paths(Path(directory), config.image_runtime_root_path)

            RuntimeMaterializer(config).materialize(
                request.resolved_runtime_image,
                paths,
                Event(),
            )

            self.assertTrue((paths.image_comfyui_root / "custom_nodes" / "websocket_image_save.py").is_file())
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
                request.resolved_runtime_image,
                paths,
                Event(),
            )

            self.assertTrue(paths.python_overlay_path.is_dir())
            self.assertFalse(stale_package.exists())
            self.assertFalse(stale_report.exists())

if __name__ == "__main__":
    unittest.main()
