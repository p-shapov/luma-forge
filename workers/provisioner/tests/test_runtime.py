import json
from pathlib import Path
import tempfile
import unittest

from helpers import start_payload
from app.errors import PreparationError
from runtime.manifest import (
    MANIFEST_KIND,
    build_manifest,
    load_manifest,
    runtime_paths,
    validate_manifest,
    write_manifest,
)
from app.schemas import parse_start_request


class RuntimeTests(unittest.TestCase):
    def test_runtime_paths_resolve_under_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))

            self.assertEqual(paths.runtime_manifest_path, Path(directory).resolve() / ".luma-forge/runtime-manifest.json")

    def test_manifest_round_trips_without_secret_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
            )

            write_manifest(manifest, paths.runtime_manifest_path)
            loaded = load_manifest(paths.runtime_manifest_path)

            self.assertEqual(loaded.manifest_kind, MANIFEST_KIND)
            payload = json.loads(paths.runtime_manifest_path.read_text(encoding="utf-8"))
            self.assertNotIn("token", payload)
            self.assertNotIn("secret", payload)
            self.assertNotIn("api_key", payload)
            self.assertNotIn("python_path", payload)
            self.assertNotIn("comfyui_root", payload)
            self.assertNotIn("image_runtime_root", payload)

    def test_manifest_requires_manifest_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))
            paths.runtime_manifest_path.parent.mkdir(parents=True)
            paths.runtime_manifest_path.write_text("{}", encoding="utf-8")

            with self.assertRaises(PreparationError):
                load_manifest(paths.runtime_manifest_path)

    def test_validate_manifest_rejects_wrong_manifest_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
            )
            invalid = type(manifest)(
                manifest_kind="container_python",
                workspace_root=manifest.workspace_root,
                model_asset_paths=manifest.model_asset_paths,
                prepared_at=manifest.prepared_at,
            )

            with self.assertRaises(PreparationError):
                validate_manifest(invalid, paths=paths)


if __name__ == "__main__":
    unittest.main()
