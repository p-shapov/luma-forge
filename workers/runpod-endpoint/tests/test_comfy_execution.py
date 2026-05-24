import base64
import json
import subprocess
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from runpod_endpoint_worker.comfy import ComfyExecutor, ComfyRuntime, parse_comfy_run_events
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import (
    ComfyLaunchError,
    ComfyNoOutputsError,
    ComfyOutputFetchError,
    ComfyOutputParseError,
    ComfyStartupError,
    ComfyWorkflowError,
    ComfyWorkflowTimeoutError,
    ResponseTooLargeError,
)
from runpod_endpoint_worker.schemas import GenerationRequest


class FakeHttpClient:
    def __init__(self, *, ready_after=1, image_body=b"image-bytes", fail_fetch=False):
        self.ready_after = ready_after
        self.image_body = image_body
        self.fail_fetch = fail_fetch
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
        if self.fail_fetch:
            raise OSError("not found")
        return self.image_body


def _write_valid_workflow(directory: str) -> Path:
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
    return workflow


def _completed_process_stdout() -> str:
    return "\n".join(
        [
            json.dumps(
                {
                    "event": "completed",
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
        ]
    )


class ComfyExecutionTests(unittest.TestCase):
    def test_runtime_launch_is_serialized_during_concurrent_cold_start(self):
        class RacingRuntime(ComfyRuntime):
            def __init__(self, *, config, http_client):
                super().__init__(config=config, http_client=http_client)
                self.barrier = threading.Barrier(2)
                self.launch_count = 0
                self.ready_checks = 0

            def _is_ready(self):
                self.ready_checks += 1
                if self.ready_checks <= 2:
                    self.barrier.wait(1)
                    return False
                return self.launch_count > 0

            def _launch(self):
                self.launch_count += 1
                time.sleep(0.01)

        runtime = RacingRuntime(
            config=EndpointConfig(comfy_ui_ready_poll_seconds=0, comfyui_startup_timeout_seconds=1),
            http_client=FakeHttpClient(),
        )
        threads = [threading.Thread(target=runtime.ensure_ready) for _ in range(2)]

        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()

        self.assertEqual(runtime.launch_count, 1)

    def test_runtime_launches_comfyui_once_and_reuses_ready_server(self):
        with tempfile.TemporaryDirectory() as directory:
            client = FakeHttpClient(ready_after=3)
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

    def test_runtime_launch_failure_does_not_include_comfy_stderr(self):
        runtime = ComfyRuntime(config=EndpointConfig(), http_client=FakeHttpClient(ready_after=1000))

        with self.assertRaises(ComfyLaunchError) as context:
            with patch("subprocess.run") as run:
                run.side_effect = subprocess.CalledProcessError(
                    returncode=1,
                    cmd=["comfy", "launch"],
                    stderr="CUDA driver unavailable",
                )
                runtime.ensure_ready()

        self.assertEqual(context.exception.code, "comfyui_launch_failed")
        self.assertEqual(context.exception.stage, "comfyui_launch")
        self.assertEqual(context.exception.message, "ComfyUI failed to launch. Process exited with status 1.")
        self.assertEqual(context.exception.metadata, {"exit_status": 1})
        self.assertNotIn("CUDA driver unavailable", context.exception.message)

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

    def test_parse_uses_terminal_outputs_without_duplicate_node_outputs(self):
        output = {
            "category": "images",
            "filename": "ComfyUI_00001_.png",
            "subfolder": "",
            "type": "output",
        }
        events = "\n".join(
            [
                json.dumps({"event": "node_executed", "outputs": [output]}),
                json.dumps({"event": "completed", "outputs": [output]}),
            ]
        )

        outputs = parse_comfy_run_events(events)

        self.assertEqual(len(outputs), 1)
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
        with self.assertRaises(ComfyOutputParseError):
            parse_comfy_run_events("{not-json}\n")

    def test_parse_rejects_missing_completion(self):
        with self.assertRaises(ComfyWorkflowError):
            parse_comfy_run_events(json.dumps({"type": "executed", "data": {"output": {"images": []}}}))

    def test_parse_rejects_missing_outputs_with_specific_code(self):
        with self.assertRaises(ComfyNoOutputsError) as context:
            parse_comfy_run_events(json.dumps({"event": "completed", "outputs": []}))

        self.assertEqual(context.exception.code, "comfyui_no_outputs")

    def test_executor_runs_patched_workflow_and_returns_base64_images(self):
        with tempfile.TemporaryDirectory() as directory:
            workflow = _write_valid_workflow(directory)
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

    def test_executor_normalizes_wildcard_host_for_local_http_fetches(self):
        with tempfile.TemporaryDirectory() as directory:
            workflow = _write_valid_workflow(directory)
            client = FakeHttpClient(image_body=b"png")
            config = EndpointConfig(workflow_path=workflow, comfyui_host="0.0.0.0")
            runtime = ComfyRuntime(config=config, http_client=client)
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=client)

            with patch.object(runtime, "ensure_ready"):
                with patch("subprocess.run") as run:
                    run.return_value = subprocess.CompletedProcess(
                        args=[],
                        returncode=0,
                        stdout="\n".join(
                            [
                                json.dumps(
                                    {
                                        "event": "completed",
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
                            ]
                        ),
                        stderr="",
                    )

                    executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

        self.assertTrue(any(url.startswith("http://127.0.0.1:") for url in client.urls))
        self.assertFalse(any(url.startswith("http://0.0.0.0:") for url in client.urls))

    def test_executor_rejects_inline_response_that_exceeds_configured_limit(self):
        with tempfile.TemporaryDirectory() as directory:
            workflow = _write_valid_workflow(directory)
            config = EndpointConfig(workflow_path=workflow, max_response_bytes=3)
            runtime = ComfyRuntime(config=config, http_client=FakeHttpClient(image_body=b"png"))
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=runtime.http_client)

            with self.assertRaises(ResponseTooLargeError):
                with patch.object(runtime, "ensure_ready"):
                    with patch("subprocess.run") as run:
                        run.return_value = subprocess.CompletedProcess(
                            args=[],
                            returncode=0,
                            stdout="\n".join(
                                [
                                    json.dumps(
                                        {
                                            "event": "completed",
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
                                ]
                            ),
                            stderr="",
                        )
                        executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

    def test_executor_workflow_failure_does_not_include_comfy_stderr(self):
        with tempfile.TemporaryDirectory() as directory:
            config = EndpointConfig(workflow_path=_write_valid_workflow(directory))
            runtime = ComfyRuntime(config=config, http_client=FakeHttpClient())
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=runtime.http_client)

            with self.assertRaises(ComfyWorkflowError) as context:
                with patch.object(runtime, "ensure_ready"):
                    with patch("subprocess.run") as run:
                        run.side_effect = subprocess.CalledProcessError(
                            returncode=1,
                            cmd=["comfy", "run"],
                            stderr="Prompt outputs failed validation",
                        )
                        executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

        self.assertEqual(context.exception.code, "comfyui_workflow_failed")
        self.assertEqual(context.exception.message, "ComfyUI workflow execution failed. Process exited with status 1.")
        self.assertEqual(context.exception.metadata, {"exit_status": 1})
        self.assertNotIn("Prompt outputs failed validation", context.exception.message)

    def test_executor_workflow_timeout_uses_specific_code(self):
        with tempfile.TemporaryDirectory() as directory:
            config = EndpointConfig(workflow_path=_write_valid_workflow(directory))
            runtime = ComfyRuntime(config=config, http_client=FakeHttpClient())
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=runtime.http_client)

            with self.assertRaises(ComfyWorkflowTimeoutError) as context:
                with patch.object(runtime, "ensure_ready"):
                    with patch("subprocess.run") as run:
                        run.side_effect = subprocess.TimeoutExpired(cmd=["comfy", "run"], timeout=1, stderr="workflow stalled")
                        executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

        self.assertEqual(context.exception.code, "comfyui_workflow_timeout")
        self.assertEqual(context.exception.message, "ComfyUI workflow execution timed out. Timed out after 1 seconds.")
        self.assertEqual(context.exception.metadata, {"timeout_seconds": 1})
        self.assertNotIn("workflow stalled", context.exception.message)

    def test_executor_output_fetch_failure_uses_specific_code(self):
        with tempfile.TemporaryDirectory() as directory:
            config = EndpointConfig(workflow_path=_write_valid_workflow(directory))
            client = FakeHttpClient(fail_fetch=True)
            runtime = ComfyRuntime(config=config, http_client=client)
            executor = ComfyExecutor(config=config, runtime=runtime, http_client=client)

            with self.assertRaises(ComfyOutputFetchError) as context:
                with patch.object(runtime, "ensure_ready"):
                    with patch("subprocess.run") as run:
                        run.return_value = subprocess.CompletedProcess(
                            args=[],
                            returncode=0,
                            stdout=_completed_process_stdout(),
                            stderr="",
                        )
                        executor.generate(GenerationRequest(execution_type="t2i", prompt="new prompt"))

        self.assertEqual(context.exception.code, "comfyui_output_fetch_failed")


if __name__ == "__main__":
    unittest.main()
