from __future__ import annotations

import ctypes
import time
from dataclasses import dataclass, field

user32 = ctypes.WinDLL("user32", use_last_error=True)

VK_MEDIA_NEXT_TRACK = 0xB0
VK_MEDIA_PREV_TRACK = 0xB1
VK_MEDIA_PLAY_PAUSE = 0xB3
KEYEVENTF_KEYUP = 0x0002

user32.keybd_event.argtypes = [ctypes.c_ubyte, ctypes.c_ubyte, ctypes.c_uint32, ctypes.c_size_t]


@dataclass
class MediaSnapshot:
    has_session: bool = False
    source_app_id: str = ""
    available_sources: list[str] = field(default_factory=list)
    title: str = ""
    artist: str = ""
    album: str = ""
    is_playing: bool = False
    position_secs: float = 0.0
    duration_secs: float = 0.0
    captured_at: float = field(default_factory=time.monotonic)
    thumbnail_bytes: bytes | None = None

    def live_position_secs(self) -> float:
        if not self.is_playing:
            return self.position_secs
        return min(max(0.0, self.position_secs + (time.monotonic() - self.captured_at)), self.duration_secs or 1e9)


def _tap_vk(vk: int) -> None:
    user32.keybd_event(vk, 0, 0, 0)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)


class SmtcController:
    def read_snapshot(self) -> MediaSnapshot:
        return MediaSnapshot()

    def toggle_play_pause(self) -> None:
        _tap_vk(VK_MEDIA_PLAY_PAUSE)

    def next_track(self) -> None:
        _tap_vk(VK_MEDIA_NEXT_TRACK)

    def prev_track(self) -> None:
        _tap_vk(VK_MEDIA_PREV_TRACK)

    def seek_to_seconds(self, _secs: float) -> None:
        pass

    def cycle_source(self) -> None:
        pass
