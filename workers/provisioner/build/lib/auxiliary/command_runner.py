from dataclasses import dataclass
from pathlib import Path
from subprocess import PIPE, STDOUT, Popen, TimeoutExpired
from threading import Event
from time import monotonic

from app.errors import PreparationError, StepTimeoutError


class Cancelled(Exception):
    pass


@dataclass(frozen=True)
class CommandRunner:
    def run(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
        error_type: type[PreparationError] = PreparationError,
    ) -> None:
        try:
            process = Popen(args, cwd=cwd, text=True)
        except OSError as error:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}") from error

        deadline = None if timeout_seconds is None else monotonic() + timeout_seconds
        while process.poll() is None:
            if cancel_event is not None and cancel_event.is_set():
                _terminate_process(process)
                raise Cancelled()
            if deadline is not None and monotonic() >= deadline:
                _terminate_process(process)
                raise StepTimeoutError("Provisioning step timed out.")
            try:
                process.wait(timeout=0.1)
            except TimeoutExpired:
                pass

        if process.returncode != 0:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}")

    def capture(
        self,
        args: list[str],
        *,
        cwd: Path | None = None,
        cancel_event: Event | None = None,
        timeout_seconds: float | None = None,
        error_type: type[PreparationError] = PreparationError,
    ) -> str:
        try:
            process = Popen(args, cwd=cwd, stdout=PIPE, stderr=STDOUT, text=True)
        except OSError as error:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}") from error

        deadline = None if timeout_seconds is None else monotonic() + timeout_seconds
        while True:
            if cancel_event is not None and cancel_event.is_set():
                _terminate_process_with_output(process)
                raise Cancelled()
            if deadline is not None and monotonic() >= deadline:
                _terminate_process_with_output(process)
                raise StepTimeoutError("Provisioning step timed out.")
            communicate_timeout = 0.1
            if deadline is not None:
                communicate_timeout = min(communicate_timeout, max(0.0, deadline - monotonic()))
            try:
                output, _ = process.communicate(timeout=communicate_timeout)
                break
            except TimeoutExpired:
                continue

        if process.returncode != 0:
            command = " ".join(args[:2])
            raise error_type(f"Command failed: {command}")
        return output


def _terminate_process(process: Popen) -> None:
    process.terminate()
    try:
        process.wait(timeout=5)
    except TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _terminate_process_with_output(process: Popen) -> None:
    process.terminate()
    try:
        process.communicate(timeout=5)
    except TimeoutExpired:
        process.kill()
        process.communicate(timeout=5)
