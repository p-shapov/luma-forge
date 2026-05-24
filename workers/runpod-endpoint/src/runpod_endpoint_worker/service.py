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

    def generate_from_payload(self, payload: Any) -> GenerationResponse:
        request = parse_generation_request(payload, self.config)
        executor = self.executor or ComfyExecutor.from_config(self.config)
        return GenerationResponse(execution_type=request.execution_type, images=executor.generate(request))
