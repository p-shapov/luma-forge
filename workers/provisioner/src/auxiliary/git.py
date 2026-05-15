from dataclasses import dataclass
from pathlib import Path
from threading import Event

from auxiliary.command_runner import CommandRunner
from app.errors import GitCheckoutError
from app.schemas import GitSource


@dataclass(frozen=True)
class GitCheckout:
    command_runner: CommandRunner
    timeout_seconds: float

    def checkout(self, source: GitSource, target: Path, cancel_event: Event) -> None:
        if target.exists():
            self.command_runner.run(
                ["git", "fetch", "--all", "--tags"],
                cwd=target,
                cancel_event=cancel_event,
                timeout_seconds=self.timeout_seconds,
                error_type=GitCheckoutError,
            )
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            self.command_runner.run(
                ["git", "clone", source.repository_url, str(target)],
                cancel_event=cancel_event,
                timeout_seconds=self.timeout_seconds,
                error_type=GitCheckoutError,
            )

        self.command_runner.run(
            ["git", "checkout", source.revision],
            cwd=target,
            cancel_event=cancel_event,
            timeout_seconds=self.timeout_seconds,
            error_type=GitCheckoutError,
        )
