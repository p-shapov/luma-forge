import os
import subprocess
import time
import unittest
from http.client import HTTPConnection


@unittest.skipUnless(os.environ.get("LUMA_FORGE_RUN_CONTAINER_SMOKE") == "1", "container smoke test is opt-in")
class ContainerSmokeTests(unittest.TestCase):
    def test_container_reports_idle_status(self):
        image = "luma-forge-provisioner:smoke"
        subprocess.run(["docker", "build", "-t", image, "."], check=True)
        container = subprocess.check_output(
            ["docker", "run", "-d", "-p", "127.0.0.1::8000", image],
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
                    connection.request("GET", "/status")
                    response = connection.getresponse()
                    payload = response.read().decode("utf-8")
                    connection.close()
                    if response.status == 200 and '"status": "idle"' in payload:
                        return
                except OSError:
                    time.sleep(1)
            self.fail(f"container did not report idle status: {payload}")
        finally:
            subprocess.run(["docker", "rm", "-f", container], check=False)


if __name__ == "__main__":
    unittest.main()
