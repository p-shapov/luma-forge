import unittest
from pathlib import Path


README = Path(__file__).resolve().parents[1] / "README.md"


class ReadmeContractTests(unittest.TestCase):
    def test_readme_documents_status_only_auto_start_contract(self):
        content = README.read_text()

        self.assertIn("auto-starts", content)
        self.assertIn("GET /status", content)
        self.assertNotIn("POST /start", content)
        self.assertNotIn("LUMA_FORGE_PROVISIONER_REQUIRES_HUGGING_FACE_API_KEY", content)
        self.assertNotIn("LUMA_FORGE_PROVISIONER_MAX_REQUEST_BYTES", content)
        self.assertIn("LUMA_FORGE_HUGGING_FACE_API_KEY", content)
        self.assertIn("Native/control-plane code owns", content)


if __name__ == "__main__":
    unittest.main()
