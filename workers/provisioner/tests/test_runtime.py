import json
from pathlib import Path
import tempfile
import unittest

from helpers import start_payload
from provisioner_worker.errors import PreparationError
from provisioner_worker.runtime import (
    ENVIRONMENT_KIND,
    build_manifest,
    load_manifest,
    runtime_paths,
    validate_manifest,
    write_manifest,
)
from provisioner_worker.schemas import parse_start_request


class RuntimeTests(unittest.TestCase):
    def test_runtime_paths_resolve_under_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))

            self.assertEqual(paths.venv_dir, Path(directory).resolve() / ".venv")
            self.assertEqual(paths.python_path, Path(directory).resolve() / ".venv/bin/python")
            self.assertEqual(paths.runtime_manifest_path, Path(directory).resolve() / ".luma-forge/runtime.json")

    def test_manifest_round_trips_without_secret_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload(Path(directory)))
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
                python_version="Python 3.12.0",
            )

            write_manifest(manifest, paths.runtime_manifest_path)
            loaded = load_manifest(paths.runtime_manifest_path)

            self.assertEqual(loaded.environment_kind, ENVIRONMENT_KIND)
            self.assertEqual(loaded.python_path, str(paths.python_path))
            payload = json.loads(paths.runtime_manifest_path.read_text(encoding="utf-8"))
            self.assertNotIn("token", payload)
            self.assertNotIn("secret", payload)
            self.assertNotIn("api_key", payload)

    def test_manifest_requires_environment_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = runtime_paths(Path(directory))
            paths.runtime_manifest_path.parent.mkdir(parents=True)
            paths.runtime_manifest_path.write_text("{}", encoding="utf-8")

            with self.assertRaises(PreparationError):
                load_manifest(paths.runtime_manifest_path)

    def test_validate_manifest_rejects_wrong_environment_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload(Path(directory)))
            paths = runtime_paths(Path(directory))
            manifest = build_manifest(
                request=request,
                paths=paths,
                python_version="Python 3.12.0",
            )
            invalid = type(manifest)(
                environment_kind="container_python",
                python_path=manifest.python_path,
                comfyui_root=manifest.comfyui_root,
                python_version=manifest.python_version,
                platform=manifest.platform,
                comfyui_revision=manifest.comfyui_revision,
                custom_node_revisions=manifest.custom_node_revisions,
                pip_freeze_path=manifest.pip_freeze_path,
                install_report_path=manifest.install_report_path,
                prepared_at=manifest.prepared_at,
            )

            with self.assertRaises(PreparationError):
                validate_manifest(invalid, paths=paths)


if __name__ == "__main__":
    unittest.main()
