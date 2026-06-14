import unittest

from app.errors import ValidationError, WorkflowValidationError
from app.schemas import (
    ExecutionSchemaInput,
    ExecutionSchemaRevision,
    GenerationRequest,
    parse_generation_request,
)


def text_to_image_schema(max_length=4000):
    return ExecutionSchemaRevision(
        version="1.0.0",
        inputs=[
            ExecutionSchemaInput(
                id="prompt",
                input_type="string",
                required=True,
                max_length=max_length,
            )
        ],
        output_type="image_set",
    )


class SchemaTests(unittest.TestCase):
    def test_accepts_valid_t2i_request(self):
        request = parse_generation_request(
            {"prompt": "a lamp"},
            text_to_image_schema(max_length=100),
            job_id="job-123",
        )

        self.assertEqual(request, GenerationRequest(inputs={"prompt": "a lamp"}, job_id="job-123"))

    def test_rejects_missing_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({}, text_to_image_schema())

    def test_rejects_unknown_input(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": "a lamp", "seed": 1}, text_to_image_schema())

    def test_rejects_blank_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": "  "}, text_to_image_schema())

    def test_rejects_non_string_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": 123}, text_to_image_schema())

    def test_rejects_oversized_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": "too long"}, text_to_image_schema(max_length=3))

    def test_rejects_secret_like_schema_input(self):
        schema = ExecutionSchemaRevision(
            version="1.0.0",
            inputs=[
                ExecutionSchemaInput(
                    id="api_key",
                    input_type="string",
                    required=True,
                    max_length=4000,
                )
            ],
            output_type="image_set",
        )

        with self.assertRaises(WorkflowValidationError):
            parse_generation_request({"api_key": "secret"}, schema)


if __name__ == "__main__":
    unittest.main()
