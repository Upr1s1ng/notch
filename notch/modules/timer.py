from __future__ import annotations

import time
from dataclasses import dataclass, field


@dataclass
class TimerState:
    total_secs: int = 25 * 60
    remaining_secs: int = 25 * 60
    running: bool = False
    completed: bool = False
    label: str = "25m Focus"
    _last_tick: float = field(default_factory=time.monotonic)

    def set_preset(self, secs: int, label: str) -> None:
        self.total_secs = max(1, secs)
        self.remaining_secs = self.total_secs
        self.running = False
        self.completed = False
        self.label = label

    def toggle_start_pause(self, now: float | None = None) -> None:
        if self.completed:
            self.reset()
        self.running = not self.running
        self._last_tick = time.monotonic() if now is None else now

    def reset(self) -> None:
        self.remaining_secs = self.total_secs
        self.running = False
        self.completed = False

    def tick(self, now: float | None = None) -> bool:
        if not self.running:
            return False
        t = time.monotonic() if now is None else now
        elapsed = int(t - self._last_tick)
        if elapsed < 1:
            return False
        self._last_tick += elapsed
        self.remaining_secs = max(0, self.remaining_secs - elapsed)
        if self.remaining_secs == 0:
            self.running = False
            self.completed = True
            return True
        return False

    def progress_fraction(self) -> float:
        return max(0.0, min(1.0, self.remaining_secs / max(1, self.total_secs)))

    def formatted_remaining(self) -> str:
        return f"{self.remaining_secs // 60:02d}:{self.remaining_secs % 60:02d}"
