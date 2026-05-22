from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.schemas import GenerationResponse, parse_generation_request


@dataclass
class GenerationService:
    config: EndpointConfig

    @classmethod
    def from_config(cls, config: EndpointConfig) -> "GenerationService":
        return cls(config=config)

    def generate_from_payload(self, payload: Any) -> GenerationResponse:
        request = parse_generation_request(payload, self.config)
        return GenerationResponse(execution_type=request.execution_type)
