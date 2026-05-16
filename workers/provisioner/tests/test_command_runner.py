import subprocess
import sys
import unittest
from threading import Event, Timer

from auxiliary.command_runner import Cancelled, CommandRunner
from app.errors import GitCheckoutError, StepTimeoutError


class CommandRunnerTests(unittest.TestCase):
    def test_interrupts_subprocess_on_cancel(self):
        cancel_event = Event()
        timer = Timer(0.2, cancel_event.set)
        timer.start()
        try:
            with self.assertRaises(Cancelled):
                CommandRunner().run(
                    [sys.executable, "-c", "import time; time.sleep(10)"],
                    cancel_event=cancel_event,
                )
        finally:
            timer.cancel()

    def test_times_out_subprocess(self):
        with self.assertRaises(StepTimeoutError):
            CommandRunner().run(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                timeout_seconds=0.1,
            )

    def test_maps_git_failures(self):
        with self.assertRaises(GitCheckoutError):
            CommandRunner().run(
                [sys.executable, "-c", "import sys; sys.exit(1)"],
                error_type=GitCheckoutError,
            )

    def test_maps_startup_failures(self):
        with self.assertRaises(GitCheckoutError):
            CommandRunner().run(
                ["missing-command-for-luma-forge-tests"],
                error_type=GitCheckoutError,
            )

    def test_run_emits_subprocess_stdout_and_stderr_to_console(self):
        completed = subprocess.run(
            [
                sys.executable,
                "-c",
                (
                    "import sys; "
                    "from auxiliary.command_runner import CommandRunner; "
                    "CommandRunner().run([sys.executable, '-c', "
                    "\"import sys; print('stdout-visible'); print('stderr-visible', file=sys.stderr)\"])"
                ),
            ],
            capture_output=True,
            check=True,
            text=True,
        )

        self.assertIn("stdout-visible", completed.stdout)
        self.assertIn("stderr-visible", completed.stderr)

    def test_capture_drains_large_stdout(self):
        output = CommandRunner().capture(
            [sys.executable, "-c", "import sys; sys.stdout.write('x' * 200000)"],
            timeout_seconds=5,
        )

        self.assertEqual(len(output), 200000)


if __name__ == "__main__":
    unittest.main()
