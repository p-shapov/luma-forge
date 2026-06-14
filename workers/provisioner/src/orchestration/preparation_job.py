from dataclasses import dataclass
from datetime import UTC, datetime
import json
from threading import Lock, Thread
from typing import TypedDict

from app import __version__
from app.config import WorkerConfig
from app.errors import ConflictError, WorkerError, WorkerErrorPayload
from orchestration.preparer import Provisioner
from app.schemas import StartRequest

ACTIVE_STATUSES = {"running"}


class JobSnapshotPayload(TypedDict):
    status: str
    phase: str | None
    progress_percent: int | None
    error: WorkerErrorPayload | None
    updated_at: str
    provisioner_version: str


@dataclass
class JobSnapshot:
    status: str
    phase: str | None
    progress_percent: int | None
    error: WorkerErrorPayload | None
    updated_at: str
    provisioner_version: str

    def to_dict(self) -> JobSnapshotPayload:
        return {
            "status": self.status,
            "phase": self.phase,
            "progress_percent": self.progress_percent,
            "error": self.error,
            "updated_at": self.updated_at,
            "provisioner_version": self.provisioner_version,
        }


class JobManager:
    def __init__(self, provisioner: Provisioner | None = None, *, config: WorkerConfig):
        self._provisioner = provisioner or Provisioner(config=config)
        self._lock = Lock()
        self._snapshot = JobSnapshot(
            status="idle",
            phase=None,
            progress_percent=None,
            error=None,
            updated_at=_now(),
            provisioner_version=__version__,
        )
        self._thread: Thread | None = None

    def status(self) -> JobSnapshot:
        with self._lock:
            return _copy_snapshot(self._snapshot)

    def start(self, request: StartRequest) -> JobSnapshot:
        with self._lock:
            if self._snapshot.status in ACTIVE_STATUSES:
                raise ConflictError(
                    "Provisioner worker already has an active job.",
                )

            self._snapshot = JobSnapshot(
                status="running",
                phase="starting",
                progress_percent=0,
                error=None,
                updated_at=_now(),
                provisioner_version=__version__,
            )
            self._thread = Thread(target=self._run, args=(request,), daemon=True)
            self._thread.start()
            _log_event(
                "provisioner_job_started",
                status=self._snapshot.status,
                phase=self._snapshot.phase,
                progress_percent=self._snapshot.progress_percent,
            )
            return _copy_snapshot(self._snapshot)

    def _run(self, request: StartRequest) -> None:
        try:
            self._provisioner.prepare(request, self._progress)
        except WorkerError as error:
            self._terminal("failed", error.to_dict())
        except Exception:
            message = "Provisioning job failed"
            self._terminal(
                "failed",
                {
                    "code": "unexpected_exception",
                    "message": message,
                },
            )
        else:
            self._terminal("succeeded", None)

    def _progress(self, phase: str, progress_percent: int | None, _message: str | None) -> None:
        with self._lock:
            self._snapshot.status = "running"
            self._snapshot.phase = phase
            self._snapshot.progress_percent = progress_percent
            self._snapshot.updated_at = _now()
            _log_event(
                "provisioner_job_progress",
                status=self._snapshot.status,
                phase=phase,
                progress_percent=progress_percent,
            )

    def _terminal(self, status: str, error: WorkerErrorPayload | None) -> None:
        with self._lock:
            self._snapshot.status = status
            self._snapshot.phase = None
            self._snapshot.progress_percent = 100 if status == "succeeded" else self._snapshot.progress_percent
            self._snapshot.error = error
            self._snapshot.updated_at = _now()
            _log_event(
                "provisioner_job_terminal",
                status=status,
                progress_percent=self._snapshot.progress_percent,
                error_code=error.get("code") if error else None,
            )


def _copy_snapshot(snapshot: JobSnapshot) -> JobSnapshot:
    return JobSnapshot(
        status=snapshot.status,
        phase=snapshot.phase,
        progress_percent=snapshot.progress_percent,
        error=snapshot.error.copy() if snapshot.error else None,
        updated_at=snapshot.updated_at,
        provisioner_version=snapshot.provisioner_version,
    )


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def _log_event(event: str, **fields: str | int | None) -> None:
    payload = {"event": event, **{key: value for key, value in fields.items() if value is not None}}
    print(json.dumps(payload, sort_keys=True), flush=True)
