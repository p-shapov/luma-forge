import os
from http.server import ThreadingHTTPServer

from provisioner_worker.api import ProvisionerRequestHandler


def main() -> None:
    host = os.environ.get("LUMA_FORGE_PROVISIONER_HOST", "127.0.0.1")
    port = int(os.environ.get("LUMA_FORGE_PROVISIONER_PORT", "8000"))
    server = ThreadingHTTPServer((host, port), ProvisionerRequestHandler)
    print(f"Provisioner Worker listening on {host}:{port}", flush=True)
    server.serve_forever()

