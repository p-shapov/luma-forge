import os
import tempfile
import unittest
from pathlib import Path
from threading import Event

from helpers import COMMIT_REVISION, start_payload, test_config
from auxiliary.command_runner import Cancelled
from app.errors import (
    DependencyInstallError,
    PreparationError,
    ValidationError,
)
from orchestration.preparer import Provisioner
from app.schemas import parse_start_request


class FakeCommandRunner:
    def __init__(self):
        self.calls = []
        self.capture_calls = []

    def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        self.calls.append((args, cwd, timeout_seconds, error_type))
        if args[0:2] == ["git", "clone"]:
            Path(args[3]).mkdir(parents=True, exist_ok=True)
            (Path(args[3]) / "main.py").write_text("", encoding="utf-8")
            (Path(args[3]) / "requirements.txt").write_text("", encoding="utf-8")
        if args[0:3] == ["python", "-m", "venv"]:
            python_path = Path(args[3]) / "bin/python"
            python_path.parent.mkdir(parents=True, exist_ok=True)
            python_path.write_text("#!/usr/bin/env python\n", encoding="utf-8")
        if "--report" in args:
            report_path = Path(args[args.index("--report") + 1])
            report_path.parent.mkdir(parents=True, exist_ok=True)
            report_path.write_text('{"install":[]}\n', encoding="utf-8")

    def capture(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        self.capture_calls.append((args, cwd, timeout_seconds, error_type))
        if args[-1] == "--version":
            return "Python 3.12.0\n"
        if args[-2:] == ["pip", "freeze"]:
            return "example==1.0.0\n"
        return ""


class CancelAwareFakeCommandRunner(FakeCommandRunner):
    def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        if cancel_event is not None and cancel_event.is_set():
            raise Cancelled()
        super().run(args, cwd=cwd, cancel_event=cancel_event, timeout_seconds=timeout_seconds, error_type=error_type)

    def capture(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
        if cancel_event is not None and cancel_event.is_set():
            raise Cancelled()
        return super().capture(
            args,
            cwd=cwd,
            cancel_event=cancel_event,
            timeout_seconds=timeout_seconds,
            error_type=error_type,
        )


class FakeDownloader:
    def __init__(self):
        self.calls = []

    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        self.calls.append((asset, target, timeout_seconds))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"model")


class CancelAwareFakeDownloader(FakeDownloader):
    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        if cancel_event is not None and cancel_event.is_set():
            raise Cancelled()
        super().download(asset, target, cancel_event=cancel_event, timeout_seconds=timeout_seconds)


class MissingFileDownloader:
    def download(self, asset, target, *, cancel_event=None, timeout_seconds=None):
        target.parent.mkdir(parents=True, exist_ok=True)


class PreparerTests(unittest.TestCase):
    def test_prepares_environment_with_mocked_git_and_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(preset=start_payload()["workflow_preset"])
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            downloader = FakeDownloader()
            phases = []

            Provisioner(
                command_runner=runner,
                downloader=downloader,
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: phases.append(phase),
                Event(),
            )

            self.assertIn("materializing_runtime", phases)
            self.assertIn("downloading_assets", phases)
            self.assertIn("validating_environment", phases)
            self.assertTrue((Path(directory) / "models/checkpoints/model.safetensors").is_file())
            self.assertFalse((Path(directory) / ".venv/bin/python").exists())
            self.assertFalse((Path(directory) / "ComfyUI").exists())
            self.assertTrue((Path(directory) / ".luma-forge/runtime-manifest.json").is_file())

    def test_uses_catalog_declared_python_overlay_path(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload(preset=start_payload()["workflow_preset"])
            payload["resolved_runtime_implementation"]["runtime_metadata"]["workspace_overlay_policy"][
                "python_overlay_path"
            ] = ".luma-forge/custom-overlay"
            request = parse_start_request(payload)
            config = test_config(workspace_mount_path=Path(directory))
            metadata_path = config.image_runtime_root_path / "runtime-metadata.json"
            metadata_path.write_text(
                metadata_path.read_text(encoding="utf-8").replace(
                    ".luma-forge/python-overlay",
                    ".luma-forge/custom-overlay",
                ),
                encoding="utf-8",
            )

            Provisioner(
                command_runner=FakeCommandRunner(),
                downloader=FakeDownloader(),
                config=config,
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            self.assertTrue((Path(directory) / ".luma-forge/custom-overlay").is_dir())
            self.assertFalse((Path(directory) / ".luma-forge/python-overlay").exists())

    def test_rejects_mutable_git_revision(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            {
                "id": "node",
                "name": "Node",
                "git_source": {
                    "source_type": "git",
                    "repository_url": "https://example.test/node.git",
                    "revision": "main",
                },
                "install": {
                    "comfyui_custom_nodes_relative_path": "custom_nodes/node",
                },
            }
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_unsafe_preset_identifiers_without_echoing_values(self):
        unsafe_values = [
            "../unsafe-token",
            " unsafe-token",
            "bad id",
            "bad/id",
            "bad<id>",
            "badé",
            "a" * 129,
        ]

        for unsafe_value in unsafe_values:
            cases = [
                ("workflow_preset.id", lambda payload: payload["workflow_preset"].update({"id": unsafe_value})),
                (
                    "custom_node.id",
                    lambda payload: payload["workflow_preset"].update(
                        {"required_custom_nodes": [custom_node(id=unsafe_value)]}
                    ),
                ),
                (
                    "model_asset.id",
                    lambda payload: payload["workflow_preset"].update(
                        {"required_model_assets": [model_asset(id=unsafe_value)]}
                    ),
                ),
            ]
            for field, mutate in cases:
                with self.subTest(field=field, unsafe_value=unsafe_value):
                    payload = start_payload()
                    mutate(payload)

                    with self.assertRaises(ValidationError) as context:
                        parse_start_request(payload)

                    self.assertNotIn(unsafe_value, str(context.exception))

    def test_rejects_unsafe_huggingface_file_path(self):
        for file_path in ["/tmp/model.safetensors", "../model.safetensors", "models/../model.safetensors"]:
            with self.subTest(file_path=file_path):
                payload = start_payload()
                payload["workflow_preset"]["required_model_assets"][0]["download_source"]["file_path"] = file_path

                with self.assertRaises(ValidationError):
                    parse_start_request(payload)

    def test_accepts_custom_node_without_requirements_path(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [custom_node()]

        request = parse_start_request(payload)

        self.assertIsNone(request.workflow_preset.required_custom_nodes[0].install.python_requirements_path)

    def test_rejects_blank_custom_node_requirements_path(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path=""),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_checkout_outside_custom_nodes(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(comfyui_custom_nodes_relative_path="models/node"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_rejects_custom_node_requirements_path_escaping_checkout_root(self):
        payload = start_payload()
        payload["workflow_preset"]["required_custom_nodes"] = [
            custom_node(python_requirements_path="../requirements.txt"),
        ]

        with self.assertRaises(ValidationError):
            parse_start_request(payload)

    def test_prepares_custom_node_under_custom_nodes_with_requirements(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            custom_node_path = (Path(directory) / "custom_nodes/example-node").resolve(strict=False)
            image_python = runner.capture_calls[0][0][0]
            overlay_path = (Path(directory) / ".luma-forge/python-overlay").resolve(strict=False)
            self.assertTrue(custom_node_path.is_dir())
            self.assertIn(
                (
                    [
                        image_python,
                        "-m",
                        "pip",
                        "install",
                        "--target",
                        str(overlay_path),
                        "--report",
                        str((Path(directory) / ".luma-forge/custom-node-example-node-install-report.json").resolve()),
                        "-r",
                        str(custom_node_path / "requirements.txt"),
                    ],
                    custom_node_path,
                    1800,
                    DependencyInstallError,
                ),
                runner.calls,
            )

    def test_uses_safe_custom_node_id_in_install_report_filename(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(id="safe.node_1", python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            report_paths = [
                Path(call[0][call[0].index("--report") + 1])
                for call in runner.calls
                if "--report" in call[0] and "custom-node" in call[0][call[0].index("--report") + 1]
            ]
            self.assertEqual(len(report_paths), 1)
            self.assertEqual(report_paths[0].parent, (Path(directory) / ".luma-forge").resolve(strict=False))
            self.assertEqual(report_paths[0].name, "custom-node-safe.node_1-install-report.json")

    def test_rejects_protected_custom_node_dependency_before_pip(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            original_run = runner.run

            def run_with_protected_requirement(args, **kwargs):
                original_run(args, **kwargs)
                if args[0:2] == ["git", "clone"]:
                    Path(args[3], "requirements.txt").write_text("torch==2.6.0\n", encoding="utf-8")

            runner.run = run_with_protected_requirement

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse(any("--target" in call[0] for call in runner.calls))

    def test_rejects_protected_custom_node_dependency_with_pep503_name_normalization(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            original_run = runner.run

            def run_with_dotted_protected_requirement(args, **kwargs):
                original_run(args, **kwargs)
                if args[0:2] == ["git", "clone"]:
                    Path(args[3], "requirements.txt").write_text(
                        "nvidia.cublas-cu12==12.6.4.1\n",
                        encoding="utf-8",
                    )

            runner.run = run_with_dotted_protected_requirement

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse(any("--target" in call[0] for call in runner.calls))

    def test_rejects_protected_dependency_from_nested_requirements_before_pip(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            original_run = runner.run

            def run_with_nested_protected_requirement(args, **kwargs):
                original_run(args, **kwargs)
                if args[0:2] == ["git", "clone"]:
                    checkout = Path(args[3])
                    (checkout / "requirements.txt").write_text("-r nested/base.txt\n", encoding="utf-8")
                    (checkout / "nested").mkdir()
                    (checkout / "nested/base.txt").write_text("torch==2.6.0\n", encoding="utf-8")

            runner.run = run_with_nested_protected_requirement

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse(any("--target" in call[0] for call in runner.calls))

    def test_rejects_protected_transitive_dependency_from_pip_report(self):
        class ProtectedTransitiveReportRunner(FakeCommandRunner):
            def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
                super().run(
                    args,
                    cwd=cwd,
                    cancel_event=cancel_event,
                    timeout_seconds=timeout_seconds,
                    error_type=error_type,
                )
                if "--report" in args:
                    report_path = Path(args[args.index("--report") + 1])
                    report_path.write_text(
                        '{"install":[{"metadata":{"name":"nvidia-cublas-cu12"}}]}\n',
                        encoding="utf-8",
                    )

        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = ProtectedTransitiveReportRunner()

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertTrue(any("--target" in call[0] for call in runner.calls))

    def test_rejects_remote_nested_requirements_before_pip(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            original_run = runner.run

            def run_with_remote_nested_requirement(args, **kwargs):
                original_run(args, **kwargs)
                if args[0:2] == ["git", "clone"]:
                    Path(args[3], "requirements.txt").write_text(
                        "-r https://example.test/requirements.txt\n",
                        encoding="utf-8",
                    )

            runner.run = run_with_remote_nested_requirement

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse(any("--target" in call[0] for call in runner.calls))

    def test_rejects_protected_direct_url_dependency_before_pip(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)
            runner = FakeCommandRunner()
            original_run = runner.run

            def run_with_protected_direct_url_requirement(args, **kwargs):
                original_run(args, **kwargs)
                if args[0:2] == ["git", "clone"]:
                    Path(args[3], "requirements.txt").write_text(
                        "git+https://example.test/torch.git#egg=torch\n",
                        encoding="utf-8",
                    )

            runner.run = run_with_protected_direct_url_requirement

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse(any("--target" in call[0] for call in runner.calls))

    def test_reports_overlay_install_failure(self):
        class FailingPipRunner(FakeCommandRunner):
            def run(self, args, *, cwd=None, cancel_event=None, timeout_seconds=None, error_type=None):
                if "--target" in args:
                    raise DependencyInstallError("install failed")
                super().run(
                    args,
                    cwd=cwd,
                    cancel_event=cancel_event,
                    timeout_seconds=timeout_seconds,
                    error_type=error_type,
                )

        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(python_requirements_path="requirements.txt"),
            ]
            request = parse_start_request(payload)

            with self.assertRaises(DependencyInstallError):
                Provisioner(
                    command_runner=FailingPipRunner(),
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

    def test_progress_messages_include_custom_node_id_and_model_asset_name(self):
        secret = "Bearer secret-token with misleading text"
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(id="safe-node", name=secret),
            ]
            payload["workflow_preset"]["required_model_assets"] = [
                model_asset(id="safe-asset", name="Inspectable Model"),
            ]
            request = parse_start_request(payload)
            messages = []

            Provisioner(
                command_runner=FakeCommandRunner(),
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: messages.append(message),
                Event(),
            )

            self.assertIn("Installing Custom Node safe-node", messages)
            self.assertIn("Downloading model asset Inspectable Model", messages)
            self.assertNotIn(secret, str(messages))

    def test_bounds_custom_node_and_asset_progress(self):
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_custom_nodes"] = [
                custom_node(
                    id=f"node-{index}",
                    comfyui_custom_nodes_relative_path=f"custom_nodes/node-{index}",
                )
                for index in range(40)
            ]
            payload["workflow_preset"]["required_model_assets"] = [
                model_asset(
                    id=f"asset-{index}",
                    comfyui_relative_path=f"models/checkpoints/model-{index}.safetensors",
                )
                for index in range(60)
            ]
            request = parse_start_request(payload)
            progress_events = []

            Provisioner(
                command_runner=FakeCommandRunner(),
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: progress_events.append((phase, progress)),
                Event(),
            )

            reported_progress = [progress for phase, progress in progress_events if progress is not None]
            custom_node_progress = [
                progress for phase, progress in progress_events if phase == "installing_custom_nodes"
            ]
            asset_progress = [progress for phase, progress in progress_events if phase == "downloading_assets"]

            self.assertTrue(all(0 <= progress <= 100 for progress in reported_progress))
            self.assertEqual(custom_node_progress[0], 30)
            self.assertEqual(custom_node_progress[-1], 55)
            self.assertEqual(asset_progress[0], 55)
            self.assertEqual(asset_progress[-1], 90)

    def test_does_not_install_base_comfyui_requirements_during_provisioning(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = FakeCommandRunner()

            Provisioner(
                command_runner=runner,
                downloader=FakeDownloader(),
                config=test_config(workspace_mount_path=Path(directory)),
            ).prepare(
                request,
                lambda phase, progress, message: None,
                Event(),
            )

            venv_path = Path(directory) / ".venv"
            self.assertFalse((venv_path / "bin/python").exists())
            self.assertNotIn(["python", "-m", "venv", str(venv_path.resolve(strict=False))], [call[0] for call in runner.calls])
            self.assertNotIn(
                [
                    str((venv_path / "bin/python").resolve(strict=False)),
                    "-m",
                    "pip",
                    "install",
                    "-r",
                    str(Path(directory) / "ComfyUI/requirements.txt"),
                ],
                [call[0] for call in runner.calls],
            )

    def test_fails_when_image_python_is_missing_during_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            config = test_config(workspace_mount_path=Path(directory))
            (config.image_runtime_root_path / ".venv/bin/python").unlink()

            with self.assertRaises(PreparationError):
                Provisioner(
                    command_runner=FakeCommandRunner(),
                    downloader=FakeDownloader(),
                    config=config,
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

    def test_does_not_write_manifest_when_final_validation_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())

            with self.assertRaises(PreparationError) as context:
                Provisioner(
                    command_runner=FakeCommandRunner(),
                    downloader=MissingFileDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())
            self.assertEqual(str(context.exception), "Model asset is missing: model")

    def test_validation_failure_includes_validated_id_but_not_preset_asset_name(self):
        secret = "Bearer secret-token"
        with tempfile.TemporaryDirectory() as directory:
            payload = start_payload()
            payload["workflow_preset"]["required_model_assets"] = [
                model_asset(id="safe-asset", name=secret),
            ]
            request = parse_start_request(payload)

            with self.assertRaises(PreparationError) as context:
                Provisioner(
                    command_runner=FakeCommandRunner(),
                    downloader=MissingFileDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    Event(),
                )

            self.assertEqual(str(context.exception), "Model asset is missing: safe-asset")
            self.assertNotIn(secret, str(context.exception))

    def test_cancels_before_comfyui_checkout_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = CancelAwareFakeCommandRunner()
            cancel_event = Event()
            cancel_event.set()

            with self.assertRaises(Cancelled):
                Provisioner(
                    command_runner=runner,
                    downloader=CancelAwareFakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(
                    request,
                    lambda phase, progress, message: None,
                    cancel_event,
                )

            self.assertEqual(runner.calls, [])
            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())

    def test_cancels_before_runtime_materialization_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = CancelAwareFakeCommandRunner()
            downloader = CancelAwareFakeDownloader()
            cancel_event = Event()
            phases = []

            def progress(phase, progress_percent, message):
                phases.append(phase)
                if phase == "materializing_runtime" and progress_percent == 5:
                    cancel_event.set()

            with self.assertRaises(Cancelled):
                Provisioner(
                    command_runner=runner,
                    downloader=downloader,
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, progress, cancel_event)

            self.assertIn("materializing_runtime", phases)
            self.assertFalse(any("--report" in call[0] for call in runner.calls))
            self.assertEqual(downloader.calls, [])
            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())

    def test_cancels_before_asset_download_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = CancelAwareFakeCommandRunner()
            downloader = CancelAwareFakeDownloader()
            cancel_event = Event()

            def progress(phase, progress_percent, message):
                if phase == "downloading_assets":
                    cancel_event.set()

            with self.assertRaises(Cancelled):
                Provisioner(
                    command_runner=runner,
                    downloader=downloader,
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, progress, cancel_event)

            self.assertEqual(downloader.calls, [])
            self.assertFalse((Path(directory) / "models/checkpoints/model.safetensors").exists())
            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())

    def test_cancels_before_final_validation_without_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            request = parse_start_request(start_payload())
            runner = CancelAwareFakeCommandRunner()
            cancel_event = Event()

            def progress(phase, progress_percent, message):
                if phase == "validating_environment" and progress_percent == 90:
                    cancel_event.set()

            with self.assertRaises(Cancelled):
                Provisioner(
                    command_runner=runner,
                    downloader=FakeDownloader(),
                    config=test_config(workspace_mount_path=Path(directory)),
                ).prepare(request, progress, cancel_event)

            self.assertTrue((Path(directory) / "models/checkpoints/model.safetensors").is_file())
            self.assertEqual(runner.capture_calls, [])
            self.assertFalse((Path(directory) / ".luma-forge/runtime-manifest.json").exists())

def custom_node(
    *,
    id="example-node",
    name="Example Node",
    comfyui_custom_nodes_relative_path="custom_nodes/example-node",
    python_requirements_path=None,
):
    install = {
        "comfyui_custom_nodes_relative_path": comfyui_custom_nodes_relative_path,
    }
    if python_requirements_path is not None:
        install["python_requirements_path"] = python_requirements_path

    return {
        "id": id,
        "name": name,
        "git_source": {
            "source_type": "git",
            "repository_url": "https://example.test/node.git",
            "revision": COMMIT_REVISION,
        },
        "install": install,
    }


def model_asset(
    *,
    id="model",
    name="Model",
    comfyui_relative_path="models/checkpoints/model.safetensors",
):
    return {
        "id": id,
        "name": name,
        "model_asset_kind": "checkpoint",
        "download_source": {
            "source_type": "huggingface",
            "repository_id": "owner/model",
            "file_path": f"{id}.safetensors",
            "revision": "main",
        },
        "install": {
            "comfyui_relative_path": comfyui_relative_path,
        },
    }


if __name__ == "__main__":
    unittest.main()
