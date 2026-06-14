from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.comfy import ComfyExecutor
from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.schemas import GenerationResponse, parse_generation_request


@dataclass
class GenerationService:
    config: EndpointConfig
    executor: Any | None = None

    @classmethod
    def from_config(cls, config: EndpointConfig) -> "GenerationService":
        return cls(config=config, executor=ComfyExecutor.from_config(config))

    def generate_from_payload(self, payload: Any, *, job_id: str = "local") -> GenerationResponse:
        request = parse_generation_request(payload, self.config, job_id=job_id)
        executor = self.executor or ComfyExecutor.from_config(self.config)
        return GenerationResponse(images=executor.generate(request))
