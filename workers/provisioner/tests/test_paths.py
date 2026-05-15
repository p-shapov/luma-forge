import tempfile
import unittest
from pathlib import Path

from app.errors import ValidationError
from auxiliary.paths import safe_child_path, safe_relative_path


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


if __name__ == "__main__":
    unittest.main()

