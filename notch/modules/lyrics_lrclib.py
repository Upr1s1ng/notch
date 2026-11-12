from __future__ import annotations

import hashlib
import json
import re
import threading
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from typing import Callable

from notch.core.config import app_data_dir

_LRC_LINE_RE = re.compile(r"^\[(\d{1,3}):(\d{2}(?:\.\d{1,3})?)\]\s*(.*)$")


@dataclass
class LrcLine:
    time_secs: float
    text: str


@dataclass
class LyricsResult:
    track_key: str
    synced: list[LrcLine] = field(default_factory=list)
    plain: list[str] = field(default_factory=list)
    loading: bool = False
    error: str = ""


def parse_lrc(lrc_text: str) -> list[LrcLine]:
    lines: list[LrcLine] = []
    for raw in lrc_text.splitlines():
        m = _LRC_LINE_RE.match(raw.strip())
        if not m:
            continue
        t = int(m.group(1)) * 60.0 + float(m.group(2))
        txt = m.group(3).strip()
        if txt:
            lines.append(LrcLine(time_secs=t, text=txt))
    lines.sort(key=lambda item: item.time_secs)
    return lines


def active_line_index(lines: list[LrcLine], pos_secs: float) -> int:
    lo, hi, best = 0, len(lines) - 1, 0
    while lo <= hi:
        mid = (lo + hi) // 2
        if lines[mid].time_secs <= pos_secs + 0.15:
            best = mid
            lo = mid + 1
        else:
            hi = mid - 1
    return best


class LyricsService:
    def __init__(self) -> None:
        self._cache_dir = app_data_dir() / "lyrics"
        self._cache_dir.mkdir(parents=True, exist_ok=True)

    def fetch_async(self, title: str, artist: str, album: str, duration_secs: float, on_ready: Callable[[LyricsResult], None]) -> None:
        key = f"{title}::{artist}::{album}".strip(":")
        cache_file = self._cache_dir / f"{hashlib.sha1(key.encode('utf-8')).hexdigest()}.json"

        def work() -> None:
            if cache_file.exists():
                try:
                    data = json.loads(cache_file.read_text(encoding="utf-8"))
                    synced = parse_lrc(data.get("syncedLyrics") or "")
                    plain = [ln.strip() for ln in (data.get("plainLyrics") or "").splitlines() if ln.strip()]
                    on_ready(LyricsResult(track_key=key, synced=synced, plain=plain))
                    return
                except Exception:
                    pass
            params = urllib.parse.urlencode({"track_name": title, "artist_name": artist, "album_name": album, "duration": str(int(duration_secs)) if duration_secs > 0 else ""})
            url = f"https://lrclib.net/api/get?{params}"
            try:
                req = urllib.request.Request(url, headers={"User-Agent": "NotchWin/1.0"})
                with urllib.request.urlopen(req, timeout=5) as resp:
                    data = json.loads(resp.read().decode("utf-8"))
                cache_file.write_text(json.dumps(data), encoding="utf-8")
                synced = parse_lrc(data.get("syncedLyrics") or "")
                plain = [ln.strip() for ln in (data.get("plainLyrics") or "").splitlines() if ln.strip()]
                on_ready(LyricsResult(track_key=key, synced=synced, plain=plain))
            except Exception as exc:
                on_ready(LyricsResult(track_key=key, error=str(exc)))

        on_ready(LyricsResult(track_key=key, loading=True))
        threading.Thread(target=work, daemon=True).start()
