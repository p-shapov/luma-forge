import importlib
import unittest


class ContainerSmokeTests(unittest.TestCase):
    def test_handler_module_imports_without_starting_generation(self):
        module = importlib.import_module("runpod_endpoint_worker.handler")

        self.assertTrue(hasattr(module, "create_handler"))


if __name__ == "__main__":
    unittest.main()
