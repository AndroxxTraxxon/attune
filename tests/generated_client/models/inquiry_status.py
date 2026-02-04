from enum import Enum

class InquiryStatus(str, Enum):
    CANCELLED = "Cancelled"
    PENDING = "Pending"
    RESPONDED = "Responded"
    TIMEOUT = "Timeout"

    def __str__(self) -> str:
        return str(self.value)
