from enum import Enum

class OwnerType(str, Enum):
    ACTION = "Action"
    IDENTITY = "Identity"
    PACK = "Pack"
    SENSOR = "Sensor"
    SYSTEM = "System"

    def __str__(self) -> str:
        return str(self.value)
