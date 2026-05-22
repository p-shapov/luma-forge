from pathlib import Path
import tempfile

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.service import GenerationService


class WorkerFixture:
    def __init__(self, *, config: EndpointConfig | None = None):
        self.tempdir = tempfile.TemporaryDirectory()
        self.workspace = Path(self.tempdir.name)
        self.config = config or EndpointConfig(
            workspace_mount_path=self.workspace,
        )
        self.service = GenerationService(config=self.config)

    def close(self):
        self.tempdir.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
