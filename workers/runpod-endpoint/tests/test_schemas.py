import unittest

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import UnsupportedExecutionTypeError, ValidationError
from runpod_endpoint_worker.schemas import parse_generation_request


class SchemaTests(unittest.TestCase):
    def test_accepts_valid_t2i_request(self):
        request = parse_generation_request(
            {"execution_type": "t2i", "prompt": "a lamp"},
            EndpointConfig(max_prompt_chars=100),
        )

        self.assertEqual(request.execution_type, "t2i")
        self.assertEqual(request.prompt, "a lamp")

    def test_rejects_missing_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"execution_type": "t2i"}, EndpointConfig())

    def test_rejects_blank_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"execution_type": "t2i", "prompt": "  "}, EndpointConfig())

    def test_rejects_non_string_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"execution_type": "t2i", "prompt": 123}, EndpointConfig())

    def test_rejects_oversized_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request(
                {"execution_type": "t2i", "prompt": "too long"},
                EndpointConfig(max_prompt_chars=3),
            )

    def test_rejects_unsupported_execution_type(self):
        with self.assertRaises(UnsupportedExecutionTypeError):
            parse_generation_request(
                {"execution_type": "i2i", "prompt": "a lamp"},
                EndpointConfig(supported_execution_types=("t2i",)),
            )


if __name__ == "__main__":
    unittest.main()
