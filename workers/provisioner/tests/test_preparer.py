import tempfile
import unittest
import sys
from pathlib import Path
from threading import Event, Timer

from helpers import start_payload
from provisioner_worker.errors import PreparationError, ValidationError
from provisioner_worker.preparer import Cancelled, CommandRunner, Provisioner, huggingface_url
from provisioner_worker.schemas import parse_start_request


class FakeCommandRunner:
    def __init__(self):
        self.calls = []

    def run(self, args, *, cwd=None, cancel_event=None):
        self.calls.append((args, cwd))
        if args[0:2] == ["git", "clone"]:
            Path(args[3]).mkdir(parents=True, exist_ok=True)
            (Path(args[3]) / "main.py").write_text("", encoding="utf-8")
            (Path(args[3]) / "requirements.txt").write_text("", encoding="utf-8")


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, url, target, *, cancel_event=None):
        self.calls.append((url, target))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class PreparerTests(unittest.TestCase):
    def test_prepares_environment_with_mocked_git_and_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(Path(directory), preset=start_payload(Path(directory))["workflow_preset"])
            payload["workflow_preset"]["required_model_assets"][0]["file_size_bytes"] = 5
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            downloader = FakeDownloader()
            phases = []

            Provisioner(command_runner=runner, downloader=downloader).prepare(
                request,
                lambda phase, progress, message: phases.append(phase),
                Event(),
            )

            self.assertIn("installing_comfyui", phases)
            self.assertIn("downloading_assets", phases)
            self.assertIn("validating_environment", phases)
            self.assertTrue((Path(directory) / "ComfyUI/models/checkpoints/model.safetensors").is_file())

    def test_fails_when_downloaded_asset_size_does_not_match(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(Path(directory))
            payload["workflow_preset"]["required_model_assets"][0]["file_size_bytes"] = 999
            request = parse_start_request(payload)

            with self.assertRaises(PreparationError):
                Provisioner(command_runner=FakeCommandRunner(), downloader=FakeDownloader()).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

    def test_builds_public_huggingface_url(self):
        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        asset = request.workflow_preset.required_model_assets[0]

        self.assertEqual(
            huggingface_url(asset),
            "https://huggingface.co/owner/model/resolve/main/model.safetensors",
        )

    def test_accepts_custom_node_without_requirements_path(self):
        payload = start_payload(Path("/tmp/workspace"))
        payload["workflow_preset"]["required_custom_nodes"] = [custom_node()]

        request = parse_start_request(payload)

        self.assertIsNone(request.workflow_preset.required_custom_nodes[0].install.python_requirements_path)

    def test_rejects_blank_custom_node_requirements_path(self):
        payload = start_payload(Path("/tmp/workspace"))
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path=""),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_checkout_outside_custom_nodes(self):
        payload = start_payload(Path("/tmp/workspace"))
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(comfyui_custom_nodes_relative_path="models/node"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_requirements_path_escaping_checkout_root(self):
        payload = start_payload(Path("/tmp/workspace"))
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path="../requirements.txt"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_prepares_custom_node_under_custom_nodes_with_requirements(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(Path(directory))
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()

            Provisioner(command_runner=runner, downloader=FakeDownloader()).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            custom_node_path = (Path(directory) / "ComfyUI/custom_nodes/example-node").resolve(strict=False)
            self.assertTrue(custom_node_path.is_dir())
            self.assertIn(
                (["python", "-m", "pip", "install", "-r", str(custom_node_path / "requirements.txt")], custom_node_path),
                runner.calls,
            )

    def test_command_runner_interrupts_subprocess_on_cancel(self):
        cancel_event = Event()
        timer = Timer(0.2, cancel_event.set)
        timer.start()
        try:
            with self.assertRaises(Cancelled):
                CommandRunner().run(
                    [sys.executable, "-c", "import time; time.sleep(10)"],
                    cancel_event=cancel_event,
                )
        finally:
            timer.cancel()


def custom_node(
    *,
    comfyui_custom_nodes_relative_path="custom_nodes/example-node",
    python_requirements_path=None,
):
    install = {
        "comfyui_custom_nodes_relative_path": comfyui_custom_nodes_relative_path,
    }
    if python_requirements_path is not None:
        install["python_requirements_path"] = python_requirements_path

    return {
        "id": "example-node",
        "name": "Example Node",
        "git_source": {
            "source_type": "git",
            "repository_url": "https://example.test/node.git",
            "revision": "main",
        },
        "install": install,
    }


if __name__ == "__main__":
    unittest.main()
