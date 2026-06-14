import json
from pathlib import Path
import tempfile

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.service import GenerationService


class WorkerFixture:
    def __init__(self, *, config: EndpointConfig | None = None, executor=None):
        self.tempdir = tempfile.TemporaryDirectory()
        self.workspace = Path(self.tempdir.name)
        self.config = config or EndpointConfig(
            workspace_mount_path=self.workspace,
            execution_schema_path=_write_schema(self.workspace),
        )
        self.service = GenerationService(config=self.config, executor=executor)

    def close(self):
        self.tempdir.cleanup()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()


def _write_schema(directory: Path) -> Path:
    path = directory / "execution-schema.json"
    path.write_text(
        json.dumps(
            {
                "version": "1.0.0",
                "inputs": [
                    {
                        "id": "prompt",
                        "type": "string",
                        "required": True,
                        "max_length": 4000,
                    }
                ],
                "outputs": {
                    "type": "image_set",
                },
            }
        ),
        encoding="utf-8",
    )
    return path
