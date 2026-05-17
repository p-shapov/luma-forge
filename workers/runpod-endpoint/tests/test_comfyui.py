import base64
import unittest

from runpod_endpoint_worker.comfyui import ComfyUiClient, ComfyUiProcessManager, render_t2i_workflow
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.environment import validate_prepared_environment
from runpod_endpoint_worker.errors import ComfyUiExecutionError, ComfyUiStartupError, ComfyUiTimeoutError, ValidationError
from runpod_endpoint_worker.schemas import GenerationRequest
from runpod_endpoint_worker.service import GenerationService
from helpers import FakeComfyUiClient, WorkerFixture


class ComfyUiTests(unittest.TestCase):
    def test_renders_prompt_placeholder(self):
        with WorkerFixture() as fixture:
            workflow = render_t2i_workflow(
                fixture.config,
                GenerationRequest(execution_type="t2i", prompt="a lamp"),
            )

        self.assertEqual(workflow["1"]["inputs"]["text"], "a lamp")

    def test_renders_configured_prompt_node(self):
        with WorkerFixture() as fixture:
            config = EndpointConfig(
                workspace_mount_path=fixture.workspace,
                image_runtime_root_path=fixture.image_runtime_root,
                t2i_prompt_node_id="1",
                t2i_prompt_input_key="text",
            )
            workflow = render_t2i_workflow(
                config,
                GenerationRequest(execution_type="t2i", prompt="a chair"),
            )

        self.assertEqual(workflow["1"]["inputs"]["text"], "a chair")

    def test_fails_when_prompt_placeholder_missing(self):
        with WorkerFixture() as fixture:
            (fixture.workspace / "workflows/t2i.json").write_text('{"1":{"inputs":{"text":"fixed"}}}', encoding="utf-8")

            with self.assertRaises(ValidationError):
                render_t2i_workflow(
                    fixture.config,
                    GenerationRequest(execution_type="t2i", prompt="a chair"),
                )

    def test_client_collects_image_output(self):
        calls = []

        def json_transport(method, path, payload, timeout):
            calls.append((method, path, payload))
            if path == "/prompt":
                return {"prompt_id": "prompt-1"}
            if path == "/history/prompt-1":
                return {
                    "prompt-1": {
                        "outputs": {
                            "9": {
                                "images": [
                                    {"filename": "image.png", "subfolder": "", "type": "output"},
                                ],
                            },
                        },
                    },
                }
            return {}

        def bytes_transport(path, timeout):
            self.assertIn("filename=image.png", path)
            return b"png"

        client = ComfyUiClient(
            base_url="http://comfy.test",
            timeout_seconds=1,
            json_transport=json_transport,
            bytes_transport=bytes_transport,
        )

        prompt_id = client.queue_prompt({"1": {}})
        image = client.wait_for_image(prompt_id)

        self.assertEqual(image.mime_type, "image/png")
        self.assertEqual(image.data, base64.b64encode(b"png").decode("ascii"))
        self.assertEqual(calls[0][0], "POST")

    def test_client_times_out_when_no_image_output_arrives(self):
        client = ComfyUiClient(
            base_url="http://comfy.test",
            timeout_seconds=0.01,
            json_transport=lambda method, path, payload, timeout: {"prompt-1": {"outputs": {}}},
            bytes_transport=lambda path, timeout: b"",
        )

        with self.assertRaises(ComfyUiTimeoutError):
            client.wait_for_image("prompt-1")

    def test_client_rejects_missing_prompt_id(self):
        client = ComfyUiClient(
            base_url="http://comfy.test",
            timeout_seconds=1,
            json_transport=lambda method, path, payload, timeout: {},
        )

        with self.assertRaises(ComfyUiExecutionError):
            client.queue_prompt({"1": {}})

    def test_service_propagates_comfyui_execution_failure(self):
        with WorkerFixture(comfyui=FakeComfyUiClient(fail_on_queue=ComfyUiExecutionError("failed"))) as fixture:
            with self.assertRaises(ComfyUiExecutionError):
                fixture.service.generate_from_payload({"execution_type": "t2i", "prompt": "a lamp"})

    def test_process_manager_starts_comfyui_lazily_and_reuses_process(self):
        with WorkerFixture() as fixture:
            client = FakeComfyUiClient(available=False)
            processes = []

            def process_factory(command, cwd, env):
                process = FakeProcess(command, cwd, env)
                processes.append(process)
                client.available = True
                return process

            manager = ComfyUiProcessManager(
                config=fixture.config,
                client=client,
                process_factory=process_factory,
                sleeper=lambda seconds: None,
            )

            runtime = validate_prepared_environment(fixture.config)
            manager.ensure_running(runtime)
            manager.ensure_running(runtime)
            manager.shutdown()

        self.assertEqual(len(processes), 1)
        self.assertEqual(processes[0].command, [
            str(fixture.venv_python),
            str(fixture.comfyui_root / "main.py"),
            "--base-directory",
            str(fixture.workspace),
            "--output-directory",
            str(fixture.workspace / "output"),
            "--listen",
            fixture.config.comfyui_host,
            "--port",
            str(fixture.config.comfyui_port),
        ])
        self.assertEqual(processes[0].cwd, fixture.comfyui_root)
        self.assertIn(str(fixture.overlay_path), processes[0].env["PYTHONPATH"])
        self.assertEqual(processes[0].env["LUMA_FORGE_CUSTOM_NODES_ROOT"], str(fixture.workspace / "custom_nodes"))
        self.assertTrue(processes[0].terminated)

    def test_process_manager_reuses_already_ready_comfyui(self):
        with WorkerFixture() as fixture:
            client = FakeComfyUiClient(available=True)

            def process_factory(command, cwd, env):
                raise AssertionError("process should not start")

            manager = ComfyUiProcessManager(
                config=fixture.config,
                client=client,
                process_factory=process_factory,
            )

            manager.ensure_running()

    def test_process_manager_fails_when_process_exits_before_ready(self):
        with WorkerFixture() as fixture:
            client = FakeComfyUiClient(available=False)

            def process_factory(command, cwd, env):
                return FakeProcess(command, cwd, env, exit_code=1)

            manager = ComfyUiProcessManager(
                config=fixture.config,
                client=client,
                process_factory=process_factory,
            )

            with self.assertRaises(ComfyUiStartupError):
                manager.ensure_running()

    def test_service_starts_comfyui_before_generation(self):
        with WorkerFixture() as fixture:
            manager = FakeProcessManager()
            service = GenerationService(
                config=fixture.config,
                comfyui=fixture.comfyui,
                process_manager=manager,
            )

            response = service.generate_from_payload({"execution_type": "t2i", "prompt": "a lamp"})

        self.assertEqual(response.image.mime_type, "image/png")
        self.assertEqual(manager.ensure_running_calls, 1)
        self.assertEqual(manager.runtime.python_path, fixture.venv_python)
        self.assertEqual(len(fixture.comfyui.queued_workflows), 1)


class FakeProcess:
    def __init__(self, command, cwd, env, *, exit_code=None):
        self.command = command
        self.cwd = cwd
        self.env = env
        self.exit_code = exit_code
        self.terminated = False
        self.killed = False

    def poll(self):
        return self.exit_code

    def terminate(self):
        self.terminated = True
        self.exit_code = 0

    def kill(self):
        self.killed = True
        self.exit_code = -9

    def wait(self, timeout=None):
        return self.exit_code or 0


class FakeProcessManager:
    def __init__(self):
        self.ensure_running_calls = 0
        self.runtime = None

    def ensure_running(self, runtime=None):
        self.ensure_running_calls += 1
        self.runtime = runtime


if __name__ == "__main__":
    unittest.main()
