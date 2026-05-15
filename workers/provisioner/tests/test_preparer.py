import tempfile
import unittest
from pathlib import Path
from threading import Event

from helpers import COMMIT_REVISION, start_payload, test_config
from app.errors import (
    DependencyInstallError,
    PreparationError,
    ValidationError,
)
from orchestration.preparer import Provisioner
from app.schemas import parse_start_request


class FakeCommandRunner:
    def __init__(self):
        self.calls = []
        self.capture_calls = []

    def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        self.calls.append((args, cwd, timeout_seconds, error_type))
        if args[0:2] == ["git", "clone"]:
            Path(args[3]).mkdir(parents=True, exist_ok=True)
            (Path(args[3]) / "main.py").write_text("", encoding="utf-8")
            (Path(args[3]) / "requirements.txt").write_text("", encoding="utf-8")
        if args[0:3] == ["python", "-m", "venv"]:
            python_path = Path(args[3]) / "bin/python"
            python_path.parent.mkdir(parents=True, exist_ok=True)
            python_path.write_text("#!/usr/bin/env python\n", encoding="utf-8")
        if "--report" in args:
            report_path = Path(args[args.index("--report") + 1])
            report_path.parent.mkdir(parents=True, exist_ok=True)
            report_path.write_text('{"install":[]}\n', encoding="utf-8")

    def capture(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        self.capture_calls.append((args, cwd, timeout_seconds, error_type))
        if args[-1] == "--version":
            return "Python 3.12.0\n"
        if args[-2:] == ["pip", "freeze"]:
            return "example==1.0.0\n"
        return ""


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        self.calls.append((asset, target, timeout_seconds))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class MissingFileDownloader:
    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        target.parent.mkdir(parents=True, exist_ok=True)


class PreparerTests(unittest.TestCase):
    def test_prepares_environment_with_mocked_git_and_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(preset=start_payload()["workflow_preset"])
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            downloader = FakeDownloader()
            phases = []

            Provisioner(
                command_runner=runner,
                downloader=downloader,
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: phases.append(phase),
                Event(),
            )

            self.assertIn("installing_comfyui", phases)
            self.assertIn("downloading_assets", phases)
            self.assertIn("validating_environment", phases)
            self.assertTrue((Path(directory) / "ComfyUI/models/checkpoints/model.safetensors").is_file())
            self.assertTrue((Path(directory) / ".venv/bin/python").is_file())
            self.assertTrue((Path(directory) / ".luma-forge/runtime.json").is_file())
            self.assertTrue((Path(directory) / ".luma-forge/pip-freeze.txt").is_file())

    def test_rejects_mutable_git_revision(self):
        payload = start_payload()
        payload["workflow_preset"]["required_comfyui_source"]["revision"] = "main"

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_unsafe_huggingface_file_path(self):
        for file_path in ["/tmp/model.safetensors", "../model.safetensors", "models/../model.safetensors"]:
            with self.subTest(file_path=file_path):
                payload = start_payload()
                payload["workflow_preset"]["required_model_assets"][0]["download_source"]["file_path"] = file_path

                with self.assertRaises(ValidationError):
                    parse_start_request(payload)

    def test_accepts_custom_node_without_requirements_path(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [custom_node()]

        request = parse_start_request(payload)

        self.assertIsNone(request.workflow_preset.required_custom_nodes[0].install.python_requirements_path)

    def test_rejects_blank_custom_node_requirements_path(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path=""),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_checkout_outside_custom_nodes(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(comfyui_custom_nodes_relative_path="models/node"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_requirements_path_escaping_checkout_root(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path="../requirements.txt"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_prepares_custom_node_under_custom_nodes_with_requirements(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            custom_node_path = (Path(directory) / "ComfyUI/custom_nodes/example-node").resolve(strict=False)
            venv_python = (Path(directory) / ".venv/bin/python").resolve(strict=False)
            self.assertTrue(custom_node_path.is_dir())
            self.assertIn(
                (
                    [
                        str(venv_python),
                        "-m",
                        "pip",
                        "install",
                        "--report",
                        str((Path(directory) / ".luma-forge/custom-node-example-node-install-report.json").resolve()),
                        "-r",
                        str(custom_node_path / "requirements.txt"),
                    ],
                    custom_node_path,
                    1800,
                    DependencyInstallError,
                ),
                runner.calls,
            )

    def test_sanitizes_custom_node_install_report_filename(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(id="../unsafe/node", python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            report_paths = [
                Path(call[0][call[0].index("--report") + 1])
                for call in runner.calls
                if "--report" in call[0] and "custom-node" in call[0][call[0].index("--report") + 1]
            ]
            self.assertEqual(len(report_paths), 1)
            self.assertEqual(report_paths[0].parent, (Path(directory) / ".luma-forge").resolve(strict=False))
            self.assertTrue(report_paths[0].name.startswith("custom-node-..-unsafe-node-"))

    def test_installs_comfyui_requirements_through_volume_venv(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            venv_path = Path(directory) / ".venv"
            venv_python = venv_path / "bin/python"
            self.assertIn(
                (["python", "-m", "venv", str(venv_path.resolve(strict=False))], None, 1800, DependencyInstallError),
                runner.calls,
            )
            self.assertIn(
                (
                    [
                        str(venv_python.resolve(strict=False)),
                        "-m",
                        "pip",
                        "install",
                        "--report",
                        str((Path(directory) / ".luma-forge/comfyui-install-report.json").resolve()),
                        "-r",
                        str((Path(directory) / "ComfyUI/requirements.txt").resolve()),
                    ],
                    (Path(directory) / "ComfyUI").resolve(strict=False),
                    1800,
                    DependencyInstallError,
                ),
                runner.calls,
            )
            self.assertNotIn(
                ["python", "-m", "pip", "install", "-r", str(Path(directory) / "ComfyUI/requirements.txt")],
                [call[0] for call in runner.calls],
            )

    def test_fails_when_volume_venv_is_missing_during_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = FakeCommandRunner()

            def skip_venv(args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
                runner.calls.append((args, cwd, timeout_seconds, error_type))
                if args[0:2] == ["git", "clone"]:
                    Path(args[3]).mkdir(parents=True, exist_ok=True)
                    (Path(args[3]) / "main.py").write_text("", encoding="utf-8")
                    (Path(args[3]) / "requirements.txt").write_text("", encoding="utf-8")
                if "--report" in args:
                    report_path = Path(args[args.index("--report") + 1])
                    report_path.parent.mkdir(parents=True, exist_ok=True)
                    report_path.write_text('{"install":[]}\n', encoding="utf-8")

            runner.run = skip_venv

            with self.assertRaises(PreparationError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

    def test_does_not_write_manifest_when_final_validation_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())

            with self.assertRaises(PreparationError):
                Provisioner(
                    command_runner=FakeCommandRunner(),
                    downloader=MissingFileDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse((Path(directory) / ".luma-forge/runtime.json").exists())

def custom_node(
    *,
    id="example-node",
    comfyui_custom_nodes_relative_path="custom_nodes/example-node",
    python_requirements_path=None,
):
    install = {
        "comfyui_custom_nodes_relative_path": comfyui_custom_nodes_relative_path,
    }
    if python_requirements_path is not None:
        install["python_requirements_path"] = python_requirements_path

    return {
        "id": id,
        "name": "Example Node",
        "git_source": {
            "source_type": "git",
            "repository_url": "https://example.test/node.git",
            "revision": COMMIT_REVISION,
        },
        "install": install,
    }

if __name__ == "__main__":
    unittest.main()
