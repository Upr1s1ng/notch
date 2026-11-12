from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from notch.os_win32.winapi import SW_SHOWNORMAL, shell32


@dataclass
class RackItem:
    path: str
    name: str
    is_dir: bool
    size_label: str


def _fmt_size(n: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024:
            return f"{n}{unit}" if unit == "B" else f"{n:.1f}{unit}"
        n /= 1024  # type: ignore[assignment]
    return f"{n:.1f}TB"


class FileRack:
    def __init__(self) -> None:
        self._items: list[RackItem] = []

    def add_paths(self, paths: list[str]) -> int:
        existing = {it.path.lower() for it in self._items}
        added = 0
        for raw in paths:
            p = Path(raw)
            if not p.exists():
                continue
            key = str(p).lower()
            if key in existing:
                continue
            existing.add(key)
            is_dir = p.is_dir()
            size_lbl = "Folder" if is_dir else _fmt_size(p.stat().st_size)
            self._items.append(RackItem(path=str(p), name=p.name or str(p), is_dir=is_dir, size_label=size_lbl))
            added += 1
        return added

    def list_items(self) -> list[RackItem]:
        return list(self._items)

    def remove_path(self, path: str) -> None:
        self._items = [it for it in self._items if it.path != path]

    def clear(self) -> None:
        self._items.clear()

    def open_item(self, path: str) -> None:
        if path:
            shell32.ShellExecuteW(None, "open", path, None, None, SW_SHOWNORMAL)
