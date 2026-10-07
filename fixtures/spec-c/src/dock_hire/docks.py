"""Docks: unlocking and returning a bicycle."""

from dataclasses import dataclass, field


@dataclass
class Dock:
    code: str
    slots: int
    bicycles: list[str] = field(default_factory=list)

    def unlock(self) -> str | None:
        """The bicycle released, or None when the dock holds none."""
        return self.bicycles.pop() if self.bicycles else None

    def give_back(self, bicycle: str) -> bool:
        """Whether a free slot took the bicycle."""
        if len(self.bicycles) >= self.slots:
            return False
        self.bicycles.append(bicycle)
        return True
