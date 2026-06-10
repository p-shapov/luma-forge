from http.server import ThreadingHTTPServer
import json
import sys

from api.handler import ProvisionerRequestHandler
from app.config import ConfigurationError, WorkerConfig
from app.errors import WorkerError
from orchestration.preparation_job import JobManager
from orchestration.preparer import Provisioner


def main() -> None:
    try:
        config = WorkerConfig.from_env()
        server = create_server(config)
    except (ConfigurationError, WorkerError) as error:
        print(json.dumps(error.to_dict()), file=sys.stderr, flush=True)
        raise SystemExit(78) from error

    print(f"Provisioner Worker listening on {config.host}:{config.port}", flush=True)
    server.serve_forever()


def create_server(config: WorkerConfig) -> ThreadingHTTPServer:
    handler = build_request_handler(config)
    # Start provisioning immediately because the control-plane expects worker progress on first poll.
    handler.manager.start(config.start_request)

    return ThreadingHTTPServer((config.host, config.port), handler)


def build_request_handler(config: WorkerConfig) -> type[ProvisionerRequestHandler]:
    manager = JobManager(Provisioner(config=config), config=config)

    class ConfiguredProvisionerRequestHandler(ProvisionerRequestHandler):
        pass

    ConfiguredProvisionerRequestHandler.config = config
    ConfiguredProvisionerRequestHandler.manager = manager
    return ConfiguredProvisionerRequestHandler
