from enum import Enum

class ExecutionStatus(str, Enum):
    ABANDONED = "Abandoned"
    CANCELING = "Canceling"
    CANCELLED = "Cancelled"
    COMPLETED = "Completed"
    FAILED = "Failed"
    REQUESTED = "Requested"
    RUNNING = "Running"
    SCHEDULED = "Scheduled"
    SCHEDULING = "Scheduling"
    TIMEOUT = "Timeout"

    def __str__(self) -> str:
        return str(self.value)
