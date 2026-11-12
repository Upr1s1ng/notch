from __future__ import annotations

import re
import sqlite3
from dataclasses import dataclass
from datetime import datetime, timedelta
from enum import Enum


class TaskFilter(str, Enum):
    TODAY = "Today"
    UPCOMING = "Upcoming"
    COMPLETED = "Done"


@dataclass
class TaskItem:
    id: int
    title: str
    due_at: float | None = None
    reminder_fired: bool = False
    done: bool = False
    sort_order: int = 0


def parse_quick_due(raw: str, now: datetime | None = None) -> tuple[str, float | None]:
    ref = now or datetime.now()
    m = re.match(r"^(.*?)(?:\s+(\d{1,2}):(\d{2}))$", raw.strip())
    if m and m.group(1).strip():
        hh, mm = int(m.group(2)), int(m.group(3))
        if 0 <= hh <= 23 and 0 <= mm <= 59:
            due = ref.replace(hour=hh, minute=mm, second=0, microsecond=0)
            if due < ref:
                due += timedelta(days=1)
            return m.group(1).strip(), due.timestamp()
    return raw.strip(), None


class TaskManager:
    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn

    def add_task(self, title: str, due_at: float | None = None) -> TaskItem:
        max_ord = self._conn.execute("SELECT COALESCE(MAX(sort_order), 0) FROM tasks").fetchone()[0]
        cur = self._conn.execute(
            "INSERT INTO tasks (title, due_at, reminder_fired, done, sort_order) VALUES (?, ?, 0, 0, ?)",
            (title, due_at, max_ord + 1),
        )
        self._conn.commit()
        return TaskItem(id=int(cur.lastrowid), title=title, due_at=due_at, sort_order=max_ord + 1)

    def list_tasks(self, task_filter: TaskFilter) -> list[TaskItem]:
        if task_filter == TaskFilter.COMPLETED:
            rows = self._conn.execute("SELECT * FROM tasks WHERE done = 1 ORDER BY sort_order ASC, id DESC").fetchall()
        elif task_filter == TaskFilter.UPCOMING:
            rows = self._conn.execute("SELECT * FROM tasks WHERE done = 0 AND due_at IS NOT NULL ORDER BY due_at ASC, sort_order ASC").fetchall()
        else:
            rows = self._conn.execute("SELECT * FROM tasks WHERE done = 0 ORDER BY sort_order ASC, id DESC").fetchall()
        return [TaskItem(id=r["id"], title=r["title"], due_at=r["due_at"], reminder_fired=bool(r["reminder_fired"]), done=bool(r["done"]), sort_order=r["sort_order"]) for r in rows]

    def toggle_done(self, task_id: int) -> None:
        self._conn.execute("UPDATE tasks SET done = 1 - done WHERE id = ?", (task_id,))
        self._conn.commit()

    def delete_task(self, task_id: int) -> None:
        self._conn.execute("DELETE FROM tasks WHERE id = ?", (task_id,))
        self._conn.commit()

    def move_task(self, task_id: int, direction: int) -> None:
        rows = self._conn.execute("SELECT id, sort_order FROM tasks WHERE done = 0 ORDER BY sort_order ASC, id DESC").fetchall()
        ids = [r["id"] for r in rows]
        if task_id not in ids:
            return
        idx = ids.index(task_id)
        swap = idx + (-1 if direction < 0 else 1)
        if 0 <= swap < len(ids):
            a_id, b_id = ids[idx], ids[swap]
            a_ord, b_ord = rows[idx]["sort_order"], rows[swap]["sort_order"]
            self._conn.execute("UPDATE tasks SET sort_order = ? WHERE id = ?", (b_ord, a_id))
            self._conn.execute("UPDATE tasks SET sort_order = ? WHERE id = ?", (a_ord, b_id))
            self._conn.commit()

    def pop_due_reminders(self, now_ts: float) -> list[TaskItem]:
        rows = self._conn.execute("SELECT * FROM tasks WHERE done = 0 AND reminder_fired = 0 AND due_at IS NOT NULL AND due_at <= ?", (now_ts,)).fetchall()
        items = [TaskItem(id=r["id"], title=r["title"], due_at=r["due_at"], reminder_fired=True, done=False, sort_order=r["sort_order"]) for r in rows]
        for it in items:
            self._conn.execute("UPDATE tasks SET reminder_fired = 1 WHERE id = ?", (it.id,))
        if items:
            self._conn.commit()
        return items
