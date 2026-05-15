from http.server import ThreadingHTTPServer
import json
import sys

from provisioner_worker.api import ProvisionerRequestHandler
from provisioner_worker.config import ConfigurationError, WorkerConfig
from provisioner_worker.job_manager import JobManager
from provisioner_worker.preparer import Provisioner


def main() -> None:
    try:
        config = WorkerConfig.from_env()
        server = create_server(config)
    except ConfigurationError as error:
        print(json.dumps(error.to_dict()), file=sys.stderr, flush=True)
        raise SystemExit(78) from error

    print(f"Provisioner Worker listening on {config.host}:{config.port}", flush=True)
    server.serve_forever()


def create_server(config: WorkerConfig) -> ThreadingHTTPServer:
    handler = build_request_handler(config)
    return ThreadingHTTPServer((config.host, config.port), handler)


def build_request_handler(config: WorkerConfig) -> type[ProvisionerRequestHandler]:
    manager = JobManager(Provisioner(config=config), config=config)

    class ConfiguredProvisionerRequestHandler(ProvisionerRequestHandler):
        pass

    ConfiguredProvisionerRequestHandler.config = config
    ConfiguredProvisionerRequestHandler.manager = manager
    return ConfiguredProvisionerRequestHandler
