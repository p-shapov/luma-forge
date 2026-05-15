import tempfile
import unittest
from pathlib import Path

from app.errors import ValidationError
from auxiliary.paths import safe_child_path, safe_custom_node_child_path, safe_relative_path
from runtime.manifest import runtime_paths


class PathSafetyTests(unittest.TestCase):
    def test_accepts_relative_path(self):
        self.assertEqual(
            safe_relative_path("models/checkpoints/model.safetensors", field_name="path"),
            Path("models/checkpoints/model.safetensors"),
        )

    def test_rejects_blank_path(self):
        with self.assertRaises(ValidationError):
            safe_relative_path(" ", field_name="path")

    def test_rejects_absolute_path(self):
        with self.assertRaises(ValidationError):
            safe_relative_path("/workspace/ComfyUI/model", field_name="path")

    def test_rejects_parent_traversal(self):
        with self.assertRaises(ValidationError):
            safe_relative_path("models/../secret", field_name="path")

    def test_rejects_path_outside_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "root"
            with self.assertRaises(ValidationError):
                safe_child_path(root, "../outside", field_name="path")

    def test_rejects_existing_symlink_escape_from_child_path(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "workspace"
            outside = Path(directory) / "outside"
            workspace.mkdir()
            outside.mkdir()
            _symlink_or_skip(self, outside, workspace / "models")

            with self.assertRaises(ValidationError):
                safe_child_path(workspace, "models/checkpoints/model.safetensors", field_name="model_path")

    def test_rejects_custom_nodes_root_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            comfyui_root = Path(directory) / "workspace" / "ComfyUI"
            outside = Path(directory) / "outside-custom-nodes"
            comfyui_root.mkdir(parents=True)
            outside.mkdir()
            _symlink_or_skip(self, outside, comfyui_root / "custom_nodes")

            with self.assertRaises(ValidationError):
                safe_custom_node_child_path(
                    comfyui_root,
                    "custom_nodes/example-node",
                    field_name="custom_node_path",
                )

    def test_rejects_custom_node_child_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            comfyui_root = Path(directory) / "workspace" / "ComfyUI"
            custom_nodes = comfyui_root / "custom_nodes"
            outside = Path(directory) / "outside-node"
            custom_nodes.mkdir(parents=True)
            outside.mkdir()
            _symlink_or_skip(self, outside, custom_nodes / "example-node")

            with self.assertRaises(ValidationError):
                safe_custom_node_child_path(
                    comfyui_root,
                    "custom_nodes/example-node/requirements.txt",
                    field_name="requirements_path",
                )

    def test_rejects_runtime_metadata_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "workspace"
            outside = Path(directory) / "outside-metadata"
            workspace.mkdir()
            outside.mkdir()
            _symlink_or_skip(self, outside, workspace / ".luma-forge")

            with self.assertRaises(ValidationError):
                runtime_paths(workspace)

    def test_rejects_runtime_venv_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory) / "workspace"
            outside = Path(directory) / "outside-venv"
            workspace.mkdir()
            outside.mkdir()
            _symlink_or_skip(self, outside, workspace / ".venv")

            with self.assertRaises(ValidationError):
                runtime_paths(workspace)

    def test_rejects_model_asset_symlink_escape(self):
        with tempfile.TemporaryDirectory() as directory:
            comfyui_root = Path(directory) / "workspace" / "ComfyUI"
            outside = Path(directory) / "outside-models"
            comfyui_root.mkdir(parents=True)
            outside.mkdir()
            _symlink_or_skip(self, outside, comfyui_root / "models")

            with self.assertRaises(ValidationError):
                safe_child_path(
                    comfyui_root,
                    "models/checkpoints/model.safetensors",
                    field_name="model_asset_path",
                )


def _symlink_or_skip(test_case: unittest.TestCase, target: Path, link: Path) -> None:
    try:
        link.symlink_to(target, target_is_directory=target.is_dir())
    except (NotImplementedError, OSError) as error:
        test_case.skipTest(f"symlinks are unavailable: {error}")


if __name__ == "__main__":
    unittest.main()
