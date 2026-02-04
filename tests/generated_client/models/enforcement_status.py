from enum import Enum

class EnforcementStatus(str, Enum):
    CREATED = "Created"
    DISABLED = "Disabled"
    PROCESSED = "Processed"

    def __str__(self) -> str:
        return str(self.value)
