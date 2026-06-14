import os
import subprocess
import time
import unittest
from http.client import HTTPConnection
from pathlib import Path


WORKERS_DIR = Path(__file__).resolve().parents[2]
REPO_ROOT = WORKERS_DIR.parent


@unittest.skipUnless(os.environ.get("LUMA_FORGE_RUN_CONTAINER_SMOKE") == "1", "container smoke test is opt-in")
class ContainerSmokeTests(unittest.TestCase):
    def test_container_reports_idle_status(self):
        image = os.environ.get("LUMA_FORGE_PROVISIONER_SMOKE_IMAGE", "provisioner:smoke")
        token = "smoke-token-0123456789abcdef0123"
        if "LUMA_FORGE_PROVISIONER_SMOKE_IMAGE" not in os.environ:
            subprocess.run(
                [
                    "docker",
                    "build",
                    "-t",
                    image,
                    "-f",
                    str(WORKERS_DIR / "provisioner/Dockerfile"),
                    str(REPO_ROOT),
                ],
                check=True,
            )
        container = subprocess.check_output(
            [
                "docker",
                "run",
                "-d",
                "-e",
                f"LUMA_FORGE_PROVISIONER_BEARER_TOKEN={token}",
                "-e",
                "LUMA_FORGE_PROVISIONER_JOB_ID=smoke-job",
                "-e",
                "LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY=false",
                "-e",
                'LUMA_FORGE_PROVISIONER_REQUIRED_MODEL_ASSETS=[]',
                "-p",
                "127.0.0.1::8000",
                image,
            ],
            text=True,
        ).strip()
        try:
            host_port = subprocess.check_output(
                ["docker", "port", container, "8000/tcp"],
                text=True,
            ).strip().rsplit(":", 1)[1]
            payload = None
            for _ in range(30):
                try:
                    connection = HTTPConnection("127.0.0.1", int(host_port), timeout=1)
                    connection.request("GET", "/status", headers={"Authorization": f"Bearer {token}"})
                    response = connection.getresponse()
                    payload = response.read().decode("utf-8")
                    connection.close()
                    if response.status == 200 and '"status": "idle"' in payload:
                        subprocess.run(
                            [
                                "docker",
                                "exec",
                                container,
                                "sh",
                                "-c",
                                "python -c 'import huggingface_hub' && test ! -e /opt/luma-forge/runtime/ComfyUI/main.py",
                            ],
                            check=True,
                        )
                        return
                except OSError:
                    time.sleep(1)
            self.fail(f"container did not report idle status: {payload}")
        finally:
            subprocess.run(["docker", "rm", "-f", container], check=False)


if __name__ == "__main__":
    unittest.main()
