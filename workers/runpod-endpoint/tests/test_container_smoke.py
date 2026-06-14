import os
import subprocess
import unittest
from pathlib import Path


WORKERS_DIR = Path(__file__).resolve().parents[2]
REPO_ROOT = WORKERS_DIR.parent


@unittest.skipUnless(os.environ.get("LUMA_FORGE_RUN_CONTAINER_SMOKE") == "1", "container smoke test is opt-in")
class ContainerSmokeTests(unittest.TestCase):
    def test_container_imports_handler_and_contains_comfy_runtime_dependencies(self):
        image = os.environ.get("LUMA_FORGE_RUNPOD_ENDPOINT_SMOKE_IMAGE", "luma-forge-runpod-endpoint:smoke")
        if "LUMA_FORGE_RUNPOD_ENDPOINT_SMOKE_IMAGE" not in os.environ:
            subprocess.run(
                [
                    "docker",
                    "build",
                    "--platform",
                    "linux/amd64",
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
                "--platform",
                "linux/amd64",
                "--rm",
                image,
                "sh",
                "-c",
                (
                    "test -f /opt/luma-forge/runtime/ComfyUI/main.py"
                    " && command -v git"
                    " && test -x /opt/luma-forge/runtime/.venv/bin/python"
                    " && test -x /opt/luma-forge/runtime/.venv/bin/comfy"
                    " && test \"$(command -v comfy)\" = /opt/luma-forge/runtime/.venv/bin/comfy"
                    " && /opt/luma-forge/runtime/.venv/bin/comfy --version"
                    " && test -f /opt/luma-forge/runtime/base-runtime/pip-freeze.txt"
                    " && test -f /opt/luma-forge/runtime/base-runtime/install-report.json"
                    " && test -s /opt/luma-forge/runtime/workflows/workflow.json"
                    " && test -s /opt/luma-forge/runtime/contracts/execution-contract.json"
                    " && test -s /opt/luma-forge/runtime/contracts/execution-schema.json"
                    " && test ! -e /opt/luma-forge/runtime/ComfyUI/custom_nodes/ComfyUI-Manager"
                    " && python -c 'from runpod_endpoint_worker.handler import build_default_handler; handler = build_default_handler(); assert handler is not None'"
                ),
            ],
            check=True,
        )


if __name__ == "__main__":
    unittest.main()
