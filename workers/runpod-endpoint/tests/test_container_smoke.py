import os
import subprocess
import unittest
from pathlib import Path


WORKERS_DIR = Path(__file__).resolve().parents[2]
REPO_ROOT = WORKERS_DIR.parent


@unittest.skipUnless(os.environ.get("LUMA_FORGE_RUN_CONTAINER_SMOKE") == "1", "container smoke test is opt-in")
class ContainerSmokeTests(unittest.TestCase):
    def test_container_imports_stub_handler_and_contains_runtime_dependencies(self):
        image = os.environ.get("LUMA_FORGE_RUNPOD_ENDPOINT_SMOKE_IMAGE", "luma-forge-runpod-endpoint:smoke")
        if "LUMA_FORGE_RUNPOD_ENDPOINT_SMOKE_IMAGE" not in os.environ:
            subprocess.run(
                [
                    "docker",
                    "build",
                    "-t",
                    image,
                    "-f",
                    str(WORKERS_DIR / "runpod-endpoint/Dockerfile"),
                    str(REPO_ROOT),
                ],
                check=True,
            )

        subprocess.run(
            [
                "docker",
                "run",
                "--rm",
                image,
                "sh",
                "-c",
                (
                    "test -f /opt/luma-forge/runtime/ComfyUI/main.py"
                    " && test -x /opt/luma-forge/runtime/.venv/bin/python"
                    " && test -f /opt/luma-forge/runtime/base-runtime/pip-freeze.txt"
                    " && test -f /opt/luma-forge/runtime/base-runtime/install-report.json"
                    " && test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager"
                    " && python -c 'from runpod_endpoint_worker.handler import create_handler; from runpod_endpoint_worker.service import GenerationService; from runpod_endpoint_worker.config import EndpointConfig; payload = create_handler(GenerationService.from_config(EndpointConfig()))({\"input\":{\"execution_type\":\"t2i\",\"prompt\":\"x\"}}); assert payload[\"generation\"][\"implemented\"] is False'"
                ),
            ],
            check=True,
        )


if __name__ == "__main__":
    unittest.main()
