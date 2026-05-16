import os
import subprocess
import time
import unittest
from http.client import HTTPConnection


@unittest.skipUnless(os.environ.get("LUMA_FORGE_RUN_CONTAINER_SMOKE") == "1", "container smoke test is opt-in")
class ContainerSmokeTests(unittest.TestCase):
    def test_container_reports_idle_status(self):
        image = os.environ.get("LUMA_FORGE_PROVISIONER_SMOKE_IMAGE", "luma-forge-provisioner:smoke")
        token = "smoke-token-0123456789abcdef0123"
        if "LUMA_FORGE_PROVISIONER_SMOKE_IMAGE" not in os.environ:
            subprocess.run(
                [
                    "docker",
                    "build",
                    "-t",
                    image,
                    "-f",
                    "../Dockerfile",
                    "--target",
                    "provisioner",
                    "../..",
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
                "LUMA_FORGE_PROVISIONER_IMAGE_REF=ghcr.io/luma-forge/provisioner-worker@sha256:1111111111111111111111111111111111111111111111111111111111111111",
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
                            ["docker", "exec", container, "python", "-c", "import huggingface_hub"],
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
