import unittest

from runpod_endpoint_worker.config import EndpointConfig
from runpod_endpoint_worker.errors import ValidationError
from runpod_endpoint_worker.schemas import parse_generation_request


class SchemaTests(unittest.TestCase):
    def test_accepts_valid_t2i_request(self):
        request = parse_generation_request(
            {"prompt": "a lamp"},
            EndpointConfig(max_prompt_chars=100),
        )

        self.assertEqual(request.prompt, "a lamp")

    def test_rejects_missing_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({}, EndpointConfig())

    def test_rejects_blank_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": "  "}, EndpointConfig())

    def test_rejects_non_string_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request({"prompt": 123}, EndpointConfig())

    def test_rejects_oversized_prompt(self):
        with self.assertRaises(ValidationError):
            parse_generation_request(
                {"prompt": "too long"},
                EndpointConfig(max_prompt_chars=3),
            )


if __name__ == "__main__":
    unittest.main()
