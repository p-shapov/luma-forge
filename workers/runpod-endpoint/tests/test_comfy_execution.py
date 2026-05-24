import base64
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from runpod_endpoint_worker.comfy import ComfyExecutor, ComfyRuntime, parse_comfy_run_events
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import ComfyExecutionError, ComfyStartupError
from runpod_endpoint_worker.schemas import GenerationRequest


class FakeHttpClient:
    def __init__(self, *, ready_after=1, image_body=b"image-bytes"):
        self.ready_after = ready_after
        self.image_body = image_body
        self.readiness_calls = 0
        self.urls = []

    def get_json(self, url, timeout):
        self.readiness_calls += 1
        self.urls.append(url)
        if self.readiness_calls < self.ready_after:
            raise OSError("not ready")
        return {"ok": True}

    def get_bytes(self, url, timeout):
        self.urls.append(url)
        return self.image_body


class ComfyExecutionTests(unittest.TestCase):
    def test_runtime_launches_comfyui_once_and_reuses_ready_server(self):
        with tempfile.TemporaryDirectory() as directory:
            client = FakeHttpClient(ready_after=2)
            config = EndpointConfig(
                comfy_ui_ready_poll_seconds=0,
                comfyui_startup_timeout_seconds=1,
                workspace_mount_path=Path(directory) / "workspace",
            )
            runtime = ComfyRuntime(config=config, http_client=client)

            with patch("subprocess.run") as run:
                runtime.ensure_ready()
                runtime.ensure_ready()

        run.assert_called_once()
        command = run.call_args.args[0]
        self.assertEqual(command[:5], [str(config.comfy_cli_path), "--skip-prompt", "--workspace", str(config.comfyui_path), "launch"])
        self.assertIn("--background", command)
        self.assertIn("--", command)
        self.assertIn("--extra-model-paths-config", command)
        extra_model_paths_config = Path(command[command.index("--extra-model-paths-config") + 1])
        self.assertTrue(extra_model_paths_config.is_file())
        self.assertIn(f"base_path: {config.workspace_mount_path}", extra_model_paths_config.read_text(encoding="utf-8"))

    def test_runtime_reuses_already_ready_local_server_without_launching(self):
        client = FakeHttpClient(ready_after=1)
        runtime = ComfyRuntime(config=EndpointConfig(), http_client=client)

        with patch("subprocess.run") as run:
            runtime.ensure_ready()

        run.assert_not_called()

    def test_runtime_reports_startup_failure_safely(self):
        client = FakeHttpClient(ready_after=1000)
        config = EndpointConfig(comfy_ui_ready_poll_seconds=0, comfyui_startup_timeout_seconds=0)
        runtime = ComfyRuntime(config=config, http_client=client)

        with self.assertRaises(ComfyStartupError):
            with patch("subprocess.run"):
                runtime.ensure_ready()

    def test_parse_completed_events_extracts_image_outputs(self):
        events = "\n".join(
            [
                json.dumps({"event": "node_progress", "value": 1}),
                json.dumps(
                    {
                        "event": "node_executed",
                        "outputs": [
                            {
                                "category": "images",
                                "filename": "ComfyUI_00001_.png",
                                "subfolder": "",
                                "type": "output",
                            }
                        ],
                    }
                ),
                json.dumps({"event": "completed"}),
            ]
        )

        outputs = parse_comfy_run_events(events)

        self.assertEqual(outputs[0].filename, "ComfyUI_00001_.png")

    def test_parse_accepts_legacy_websocket_shaped_events(self):
        events = "\n".join(
            [
                json.dumps(
                    {
                        "type": "executed",
                        "data": {
                            "output": {
                                "images": [
                                    {
                                        "filename": "ComfyUI_00001_.png",
                                        "subfolder": "",
                                        "type": "output",
                                    }
                                ]
                            }
                        },
                    }
                ),
                json.dumps({"type": "execution_success"}),
            ]
        )

        outputs = parse_comfy_run_events(events)

        self.assertEqual(outputs[0].filename, "ComfyUI_00001_.png")

    def test_parse_rejects_malformed_events(self):
        with self.assertRaises(ComfyExecutionError):
            parse_comfy_run_events("{not-json}\n")

    def test_parse_rejects_missing_completion(self):
        with self.assertRaises(ComfyExecutionError):
            parse_comfy_run_events(json.dumps({"type": "executed", "data": {"output": {"images": []}}}))

    def test_executor_runs_patched_workflow_and_returns_base64_images(self):
        with tempfile.TemporaryDirectory() as directory:
            workflow = Path(directory) / "workflow.json"
            workflow.write_text(
                json.dumps(
                    {
                        "nodes": [
                            {"id": 171, "type": "PrimitiveStringMultiline", "title": "User Prompt", "widgets_values": ["old"]},
                            {"id": 154, "type": "PrimitiveBoolean", "title": "Switch to Image Edit", "widgets_values": [True]},
                            {"id": 177, "type": "PrimitiveBoolean", "title": "Enable Prompt Refine?", "widgets_values": [True]},
                            {"id": 227, "type": "SaveImage", "widgets_values": ["hidream_o1"]},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            config = EndpointConfig(workflow_path=workflow)
            runtime = ComfyRuntime(config=config, http_client=FakeHttpClient(image_body=b"png"))
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=runtime.http_client)

            with patch.object(runtime, "ensure_ready") as ready:
                with patch("subprocess.run") as run:
                    run.return_value = subprocess.CompletedProcess(
                        args=[],
                        returncode=0,
                        stdout="\n".join(
                            [
                                json.dumps(
                                    {
                                        "event": "node_executed",
                                        "outputs": [
                                            {
                                                "category": "images",
                                                "filename": "ComfyUI_00001_.png",
                                                "subfolder": "",
                                                "type": "output",
                                            }
                                        ],
                                    }
                                ),
                                json.dumps({"event": "completed"}),
                            ]
                        ),
                        stderr="",
                    )

                    images = executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

        ready.assert_called_once()
        command = run.call_args.args[0]
        self.assertIn("--host", command)
        self.assertIn(config.comfyui_host, command)
        self.assertIn("--port", command)
        self.assertIn(str(config.comfyui_port), command)
        self.assertIn("--timeout", command)
        self.assertIn(str(config.execution_timeout_seconds), command)
        self.assertNotIn("--address", command)
        self.assertIn("--wait", command)
        self.assertEqual(images[0].data_base64, base64.b64encode(b"png").decode("ascii"))
        self.assertEqual(images[0].mime_type, "image/png")
        self.assertEqual(images[0].filename, "ComfyUI_00001_.png")


if __name__ == "__main__":
    unittest.main()
