import json
from dataclasses import dataclass
from typing import Any

from app.config import EndpointConfig
from app.errors import WorkflowValidationError
from app.schemas import (
    ExecutionSchemaInput,
    ExecutionSchemaRevision,
    GenerationResponse,
    parse_generation_request,
)
from runtime.comfy import ComfyExecutor


def load_execution_schema(path) -> ExecutionSchemaRevision:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkflowValidationError("Baked execution contract could not be loaded.") from error
    if not isinstance(value, dict):
        raise WorkflowValidationError("Baked execution contract must be a JSON object.")
    schema = value.get("execution_schema")
    if not isinstance(schema, dict):
        raise WorkflowValidationError("Baked execution contract schema is invalid.")
    outputs = schema.get("outputs")
    inputs = schema.get("inputs")
    if not isinstance(outputs, dict) or not isinstance(inputs, list):
        raise WorkflowValidationError("Baked execution contract schema is invalid.")
    parsed_inputs = []
    for input in inputs:
        if not isinstance(input, dict):
            raise WorkflowValidationError("Baked execution contract schema input is invalid.")
        parsed_inputs.append(
            ExecutionSchemaInput(
                id=input.get("id") if isinstance(input.get("id"), str) else "",
                input_type=input.get("type") if isinstance(input.get("type"), str) else "",
                required=input.get("required") is True,
                max_length=input.get("max_length") if isinstance(input.get("max_length"), int) else None,
            )
        )
    return ExecutionSchemaRevision(
        version=schema.get("version") if isinstance(schema.get("version"), str) else "",
        inputs=parsed_inputs,
        output_type=outputs.get("type") if isinstance(outputs.get("type"), str) else "",
    )


@dataclass
class GenerationService:
    config: EndpointConfig
    executor: Any | None = None

    @classmethod
    def from_config(cls, config: EndpointConfig) -> "GenerationService":
        return cls(config=config, executor=ComfyExecutor.from_config(config))

    def generate_from_payload(self, payload: Any, *, job_id: str = "local") -> GenerationResponse:
        schema = load_execution_schema(self.config.execution_contract_path)
        request = parse_generation_request(payload, schema, job_id=job_id)
        executor = self.executor or ComfyExecutor.from_config(self.config)
        return GenerationResponse(images=executor.generate(request))
