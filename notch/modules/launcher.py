from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path

from notch.os_win32.winapi import SW_SHOWNORMAL, shell32


@dataclass
class AppShortcut:
    id: int
    name: str
    target: str
    sort_order: int = 0


class ShortcutManager:
    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn

    def list_shortcuts(self) -> list[AppShortcut]:
        rows = self._conn.execute("SELECT * FROM shortcuts ORDER BY sort_order ASC, id ASC").fetchall()
        return [AppShortcut(id=r["id"], name=r["name"], target=r["target"], sort_order=r["sort_order"]) for r in rows]

    def pin_shortcut(self, target: str, name: str | None = None) -> AppShortcut:
        clean = target.strip().strip('"')
        label = name or (clean.split("//")[-1].split("/")[0] if clean.startswith(("http://", "https://")) else Path(clean).stem or clean)
        max_ord = self._conn.execute("SELECT COALESCE(MAX(sort_order), 0) FROM shortcuts").fetchone()[0]
        cur = self._conn.execute("INSERT INTO shortcuts (name, target, sort_order) VALUES (?, ?, ?)", (label[:24], clean, max_ord + 1))
        self._conn.commit()
        return AppShortcut(id=int(cur.lastrowid), name=label[:24], target=clean, sort_order=max_ord + 1)

    def remove_shortcut(self, shortcut_id: int) -> None:
        self._conn.execute("DELETE FROM shortcuts WHERE id = ?", (shortcut_id,))
        self._conn.commit()

    def launch(self, target: str) -> None:
        if target:
            shell32.ShellExecuteW(None, "open", target, None, None, SW_SHOWNORMAL)
