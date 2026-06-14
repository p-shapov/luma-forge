from dataclasses import dataclass
from typing import Any

from runpod_endpoint_worker.errors import ValidationError, WorkflowValidationError


@dataclass(frozen=True)
class ExecutionSchemaInput:
    id: str
    input_type: str
    required: bool
    max_length: int | None = None


@dataclass(frozen=True)
class ExecutionSchemaRevision:
    version: str
    inputs: list[ExecutionSchemaInput]
    output_type: str


@dataclass(frozen=True)
class GenerationRequest:
    inputs: dict[str, Any]
    job_id: str = "local"


@dataclass(frozen=True)
class GenerationImage:
    filename: str
    mime_type: str
    byte_size: int
    sha256: str
    artifact_uri: str
    storage_type: str
    relative_path: str

    def to_payload(self) -> dict[str, Any]:
        return {
            "filename": self.filename,
            "mime_type": self.mime_type,
            "byte_size": self.byte_size,
            "sha256": self.sha256,
            "artifact_uri": self.artifact_uri,
            "storage": {
                "type": self.storage_type,
                "relative_path": self.relative_path,
            },
        }


@dataclass(frozen=True)
class GenerationResponse:
    images: list[GenerationImage]

    def to_payload(self) -> dict[str, Any]:
        return {
            "status": "succeeded",
            "generation": {
                "implemented": True,
                "images": [image.to_payload() for image in self.images],
            },
        }


def parse_generation_request(payload: Any, schema: ExecutionSchemaRevision, *, job_id: str = "local") -> GenerationRequest:
    _validate_schema(schema)
    data = _object(payload, "input")
    allowed = {input.id: input for input in schema.inputs}
    unknown = sorted(set(data) - set(allowed))
    if unknown:
        raise ValidationError(f"unknown input: {unknown[0]}")

    parsed: dict[str, Any] = {}
    for input in schema.inputs:
        value = data.get(input.id)
        if value is None:
            if input.required:
                raise ValidationError(f"{input.id} is required")
            continue
        if input.input_type == "string":
            text = _non_empty_string(value, input.id)
            if input.max_length is not None and len(text) > input.max_length:
                raise ValidationError(f"{input.id} is too large")
            parsed[input.id] = text
            continue
        raise ValidationError(f"{input.id} has unsupported type")

    return GenerationRequest(inputs=parsed, job_id=job_id)


def _validate_schema(schema: ExecutionSchemaRevision) -> None:
    if schema.version.strip() == "" or schema.output_type.strip() == "":
        raise WorkflowValidationError("Baked execution schema is invalid.")
    seen = set()
    for input in schema.inputs:
        if (
            input.id.strip() == ""
            or input.id in seen
            or _is_secret_like(input.id)
            or input.input_type != "string"
            or input.max_length == 0
        ):
            raise WorkflowValidationError("Baked execution schema input is invalid.")
        seen.add(input.id)


def _is_secret_like(value: str) -> bool:
    lowered = value.lower()
    return any(marker in lowered for marker in ("secret", "token", "password", "api_key", "apikey", "credential"))


def _object(payload: Any, field: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise ValidationError(f"{field} must be an object")
    return payload


def _non_empty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or value.strip() == "":
        raise ValidationError(f"{field} must be a non-empty string")
    return value.strip()
