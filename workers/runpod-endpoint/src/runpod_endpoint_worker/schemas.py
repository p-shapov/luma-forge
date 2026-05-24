from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import UnsupportedExecutionTypeError, ValidationError


@dataclass(frozen=True)
class GenerationRequest:
    execution_type: str
    prompt: str


@dataclass(frozen=True)
class GenerationImage:
    filename: str
    mime_type: str
    data_base64: str

    def to_payload(self) -> dict[str, str]:
        return {
            "filename": self.filename,
            "mime_type": self.mime_type,
            "data_base64": self.data_base64,
        }


@dataclass(frozen=True)
class GenerationResponse:
    execution_type: str
    images: list[GenerationImage]

    def to_payload(self) -> dict[str, Any]:
        return {
            "status": "succeeded",
            "generation": {
                "implemented": True,
                "execution_type": self.execution_type,
                "images": [image.to_payload() for image in self.images],
            },
        }


def parse_generation_request(payload: Any, config: EndpointConfig) -> GenerationRequest:
    data = _object(payload, "input")
    execution_type = _non_empty_string(data.get("execution_type"), "execution_type")
    if execution_type not in config.supported_execution_types:
        raise UnsupportedExecutionTypeError("Execution type is not supported.")

    prompt = _non_empty_string(data.get("prompt"), "prompt")
    if len(prompt) > config.max_prompt_chars:
        raise ValidationError("prompt is too large")

    return GenerationRequest(execution_type=execution_type, prompt=prompt)


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()
