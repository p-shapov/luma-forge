from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import string
from threading import Event

from auxiliary.command_runner import CommandRunner
from app.errors import DependencyInstallError
from runtime.manifest import RuntimePaths


@dataclass(frozen=True)
class PythonEnvironment:
    command_runner: CommandRunner
    timeout_seconds: float

    def ensure_volume_venv(self, venv_dir: Path, cancel_event: Event) -> None:
        if (venv_dir / "bin" / "python").is_file():
            return
        venv_dir.parent.mkdir(parents=True, exist_ok=True)
        self.command_runner.run(
            ["python", "-m", "venv", str(venv_dir)],
            cancel_event=cancel_event,
            timeout_seconds=self.timeout_seconds,
            error_type=DependencyInstallError,
        )

    def install_requirements(
        self,
        requirements_path: Path,
        *,
        cwd: Path,
        python_path: Path,
        report_label: str,
        metadata_dir: Path,
        cancel_event: Event,
    ) -> None:
        if requirements_path.exists():
            metadata_dir.mkdir(parents=True, exist_ok=True)
            report_path = metadata_report_path(metadata_dir, report_label)
            self.command_runner.run(
                [
                    str(python_path),
                    "-m",
                    "pip",
                    "install",
                    "--report",
                    str(report_path),
                    "-r",
                    str(requirements_path),
                ],
                cwd=cwd,
                cancel_event=cancel_event,
                timeout_seconds=self.timeout_seconds,
                error_type=DependencyInstallError,
            )

    def write_dependency_records(self, paths: RuntimePaths, cancel_event: Event) -> None:
        paths.metadata_dir.mkdir(parents=True, exist_ok=True)
        freeze = self.command_runner.capture(
            [str(paths.python_path), "-m", "pip", "freeze"],
            cancel_event=cancel_event,
            timeout_seconds=self.timeout_seconds,
            error_type=DependencyInstallError,
        )
        paths.pip_freeze_path.write_text(freeze, encoding="utf-8")
        install_reports = sorted(path.name for path in paths.metadata_dir.glob("*-install-report.json"))
        paths.install_report_path.write_text(
            json.dumps({"reports": install_reports}, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def capture_python_version(self, python_path: Path, cancel_event: Event) -> str:
        return self.command_runner.capture(
            [str(python_path), "--version"],
            cancel_event=cancel_event,
            timeout_seconds=self.timeout_seconds,
            error_type=DependencyInstallError,
        ).strip()


def metadata_report_path(metadata_dir: Path, report_label: str) -> Path:
    safe_chars = string.ascii_letters + string.digits + "._-"
    safe_label = "".join(character if character in safe_chars else "-" for character in report_label).strip(".-_")
    if safe_label == "":
        raise DependencyInstallError("Dependency install report label is invalid.")
    if safe_label != report_label:
        digest = hashlib.sha256(report_label.encode("utf-8")).hexdigest()[:12]
        safe_label = f"{safe_label}-{digest}"

    metadata_root = metadata_dir.resolve(strict=False)
    report_path = (metadata_root / f"{safe_label}-install-report.json").resolve(strict=False)
    if report_path.parent != metadata_root:
        raise DependencyInstallError("Dependency install report path is invalid.")
    return report_path
