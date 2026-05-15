from dataclasses import dataclass
from datetime import UTC, datetime
from threading import Event, Lock, Thread
from typing import TypedDict

from provisioner_worker import __version__
from provisioner_worker.config import WorkerConfig
from provisioner_worker.errors import ConflictError, ValidationError, WorkerError, WorkerErrorPayload
from provisioner_worker.preparer import Cancelled, Provisioner
from provisioner_worker.schemas import CancelRequest, StartRequest

ACTIVE_STATUSES = {"running", "cancelling"}


class JobSnapshotPayload(TypedDict):
    status: str
    job_id: str | None
    phase: str | None
    progress_percent: int | None
    diagnostic_message: str | None
    error: WorkerErrorPayload | None
    updated_at: str
    provisioner_version: str


@dataclass
class JobSnapshot:
    status: str
    job_id: str | None
    phase: str | None
    progress_percent: int | None
    diagnostic_message: str | None
    error: WorkerErrorPayload | None
    updated_at: str
    provisioner_version: str

    def to_dict(self) -> JobSnapshotPayload:
        return {
            "status": self.status,
            "job_id": self.job_id,
            "phase": self.phase,
            "progress_percent": self.progress_percent,
            "diagnostic_message": self.diagnostic_message,
            "error": self.error,
            "updated_at": self.updated_at,
            "provisioner_version": self.provisioner_version,
        }


class JobManager:
    def __init__(self, provisioner: Provisioner | None = None, *, config: WorkerConfig):
        self._provisioner = provisioner or Provisioner(config=config)
        self._lock = Lock()
        self._cancel_event = Event()
        self._snapshot = JobSnapshot(
            status="idle",
            job_id=None,
            phase=None,
            progress_percent=None,
            diagnostic_message=None,
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
                    context={"active_job_id": self._snapshot.job_id},
                )

            self._cancel_event = Event()
            self._snapshot = JobSnapshot(
                status="running",
                job_id=request.job_id,
                phase="starting",
                progress_percent=0,
                diagnostic_message="Provisioning job accepted",
                error=None,
                updated_at=_now(),
                provisioner_version=__version__,
            )
            self._thread = Thread(target=self._run, args=(request, self._cancel_event), daemon=True)
            self._thread.start()
            return _copy_snapshot(self._snapshot)

    def cancel(self, request: CancelRequest) -> JobSnapshot:
        with self._lock:
            if self._snapshot.job_id != request.job_id or self._snapshot.status not in ACTIVE_STATUSES:
                raise ValidationError(
                    "No matching active job to cancel.",
                    reason_code="no_matching_active_job",
                    context={"field": "job_id"},
                )
            self._snapshot.status = "cancelling"
            self._snapshot.diagnostic_message = "Cancellation requested"
            self._snapshot.updated_at = _now()
            self._cancel_event.set()
            return _copy_snapshot(self._snapshot)

    def _run(self, request: StartRequest, cancel_event: Event) -> None:
        try:
            self._provisioner.prepare(request, self._progress, cancel_event)
        except Cancelled:
            self._terminal("cancelled", "Provisioning job cancelled", None)
        except WorkerError as error:
            self._terminal("failed", error.message, error.to_dict())
        except Exception as error:
            message = "Provisioning job failed"
            self._terminal(
                "failed",
                message,
                {
                    "code": "unexpected_error",
                    "reason_code": "unexpected_exception",
                    "message": message,
                },
            )
            raise error
        else:
            if cancel_event.is_set():
                self._terminal("cancelled", "Provisioning job cancelled", None)
            else:
                self._terminal("succeeded", "Provisioning job succeeded", None)

    def _progress(self, phase: str, progress_percent: int | None, message: str | None) -> None:
        with self._lock:
            if self._snapshot.status == "cancelling":
                return
            self._snapshot.status = "running"
            self._snapshot.phase = phase
            self._snapshot.progress_percent = progress_percent
            self._snapshot.diagnostic_message = message
            self._snapshot.updated_at = _now()

    def _terminal(self, status: str, message: str, error: WorkerErrorPayload | None) -> None:
        with self._lock:
            self._snapshot.status = status
            self._snapshot.phase = None
            self._snapshot.progress_percent = 100 if status == "succeeded" else self._snapshot.progress_percent
            self._snapshot.diagnostic_message = message
            self._snapshot.error = error
            self._snapshot.updated_at = _now()


def _copy_snapshot(snapshot: JobSnapshot) -> JobSnapshot:
    return JobSnapshot(
        status=snapshot.status,
        job_id=snapshot.job_id,
        phase=snapshot.phase,
        progress_percent=snapshot.progress_percent,
        diagnostic_message=snapshot.diagnostic_message,
        error=snapshot.error.copy() if snapshot.error else None,
        updated_at=snapshot.updated_at,
        provisioner_version=snapshot.provisioner_version,
    )


def _now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")
