import tempfile
import unittest
import sys
import time
from pathlib import Path
from threading import Event, Timer

from helpers import COMMIT_REVISION, start_payload
from provisioner_worker.errors import (
    AssetAuthRequiredError,
    AssetDownloadError,
    DependencyInstallError,
    GitCheckoutError,
    StepTimeoutError,
    ValidationError,
)
from provisioner_worker.preparer import Cancelled, CommandRunner, Provisioner, PublicFileDownloader
from provisioner_worker.schemas import parse_start_request


class FakeCommandRunner:
    def __init__(self):
        self.calls = []

    def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        self.calls.append((args, cwd, timeout_seconds, error_type))
        if args[0:2] == ["git", "clone"]:
            Path(args[3]).mkdir(parents=True, exist_ok=True)
            (Path(args[3]) / "main.py").write_text("", encoding="utf-8")
            (Path(args[3]) / "requirements.txt").write_text("", encoding="utf-8")


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        self.calls.append((asset, target, timeout_seconds))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class FakeHubDownload:
    def __init__(self):
        self.calls = []

    def __call__(self, **kwargs):
        self.calls.append(kwargs)
        local_path = Path(kwargs["local_dir"]) / kwargs["filename"]
        local_path.parent.mkdir(parents=True, exist_ok=True)
        local_path.write_bytes(b"model")
        return str(local_path)


class SlowHubDownload:
    def __call__(self, **kwargs):
        time.sleep(10)
        local_path = Path(kwargs["local_dir"]) / kwargs["filename"]
        local_path.parent.mkdir(parents=True, exist_ok=True)
        local_path.write_bytes(b"late")
        return str(local_path)


class PreparerTests(unittest.TestCase):
    def test_prepares_environment_with_mocked_git_and_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(Path(directory), preset=start_payload(Path(directory))["workflow_preset"])
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

    def test_downloads_asset_with_huggingface_hub_client(self):
        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            hub_download = FakeHubDownload()

            PublicFileDownloader(hub_download).download(asset, target, cancel_event=Event(), timeout_seconds=None)

            self.assertEqual(hub_download.calls[0]["repo_id"], "owner/model")
            self.assertEqual(hub_download.calls[0]["filename"], "model.safetensors")
            self.assertEqual(hub_download.calls[0]["revision"], "main")
            self.assertFalse(hub_download.calls[0]["token"])
            self.assertTrue(target.is_file())

    def test_uses_hub_returned_target_for_cache_reuse(self):
        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"cached")
            calls = []

            def cached_download(**kwargs):
                calls.append(kwargs)
                return str(target)

            PublicFileDownloader(cached_download).download(asset, target, cancel_event=Event(), timeout_seconds=None)

            self.assertEqual(len(calls), 1)
            self.assertEqual(target.read_bytes(), b"cached")

    def test_maps_huggingface_auth_failure(self):
        def fail_auth(**kwargs):
            error = RuntimeError("forbidden")
            error.response = type("Response", (), {"status_code": 403})()
            raise error

        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetAuthRequiredError):
                PublicFileDownloader(fail_auth).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    cancel_event=Event(),
                    timeout_seconds=None,
                )

    def test_maps_huggingface_download_failure(self):
        def fail_download(**kwargs):
            raise RuntimeError("missing")

        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(AssetDownloadError):
                PublicFileDownloader(fail_download).download(
                    request.workflow_preset.required_model_assets[0],
                    Path(directory) / "model.safetensors",
                    cancel_event=Event(),
                    timeout_seconds=None,
                )

    def test_download_timeout_terminates_hub_process(self):
        request = parse_start_request(start_payload(Path("/tmp/workspace")))
        asset = request.workflow_preset.required_model_assets[0]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "models/checkpoints/model.safetensors"

            with self.assertRaises(StepTimeoutError):
                PublicFileDownloader(SlowHubDownload()).download(
                    asset,
                    target,
                    cancel_event=Event(),
                    timeout_seconds=0.1,
                )

            time.sleep(0.3)
            self.assertFalse(target.exists())

    def test_rejects_mutable_git_revision(self):
        payload = start_payload(Path("/tmp/workspace"))
        payload["workflow_preset"]["required_comfyui_source"]["revision"] = "main"

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_unsafe_huggingface_file_path(self):
        for file_path in ["/tmp/model.safetensors", "../model.safetensors", "models/../model.safetensors"]:
            with self.subTest(file_path=file_path):
                payload = start_payload(Path("/tmp/workspace"))
                payload["workflow_preset"]["required_model_assets"][0]["download_source"]["file_path"] = file_path

                with self.assertRaises(ValidationError):
                    parse_start_request(payload)

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
                (
                    ["python", "-m", "pip", "install", "-r", str(custom_node_path / "requirements.txt")],
                    custom_node_path,
                    1800,
                    DependencyInstallError,
                ),
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

    def test_command_runner_times_out_subprocess(self):
        with self.assertRaises(StepTimeoutError):
            CommandRunner().run(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                timeout_seconds=0.1,
            )

    def test_command_runner_maps_git_failures(self):
        with self.assertRaises(GitCheckoutError):
            CommandRunner().run(
                [sys.executable, "-c", "import sys; sys.exit(1)"],
                error_type=GitCheckoutError,
            )

    def test_command_runner_maps_startup_failures(self):
        with self.assertRaises(GitCheckoutError):
            CommandRunner().run(
                ["missing-command-for-luma-forge-tests"],
                error_type=GitCheckoutError,
            )


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
            "revision": COMMIT_REVISION,
        },
        "install": install,
    }

if __name__ == "__main__":
    unittest.main()
