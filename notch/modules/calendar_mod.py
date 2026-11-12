from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

from notch.modules.tasks import TaskFilter, TaskManager


@dataclass
class AgendaItem:
    starts_at: float
    summary: str
    source: str = "task"


class CalendarManager:
    def __init__(self, tasks: TaskManager) -> None:
        self._tasks = tasks
        self._ics_events: list[AgendaItem] = []

    def import_ics_file(self, path: Path) -> int:
        text = path.read_text(encoding="utf-8", errors="ignore")
        added = 0
        summary = ""
        dtstart: float | None = None
        for line in text.splitlines():
            ln = line.strip()
            if ln == "BEGIN:VEVENT":
                summary, dtstart = "", None
            elif ln.startswith("SUMMARY:"):
                summary = ln[8:].strip()
            elif ln.startswith("DTSTART"):
                raw = ln.split(":", 1)[-1].strip().rstrip("Z")
                for fmt in ("%Y%m%dT%H%M%S", "%Y%m%d"):
                    try:
                        dtstart = datetime.strptime(raw[:15], fmt).timestamp()
                        break
                    except ValueError:
                        continue
            elif ln == "END:VEVENT" and summary and dtstart is not None:
                self._ics_events.append(AgendaItem(starts_at=dtstart, summary=summary, source="ics"))
                added += 1
        return added

    def get_agenda(self) -> list[AgendaItem]:
        items = list(self._ics_events)
        for t in self._tasks.list_tasks(TaskFilter.UPCOMING):
            if t.due_at is not None:
                items.append(AgendaItem(starts_at=t.due_at, summary=t.title, source="task"))
        items.sort(key=lambda i: i.starts_at)
        return items
