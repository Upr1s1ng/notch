from __future__ import annotations

import json
import os
from dataclasses import asdict, dataclass
from enum import Enum
from pathlib import Path

APP_DIR_NAME = "Notch"
CONFIG_FILE_NAME = "config.json"


class VisibilityMode(str, Enum):
    FIXED = "fixed"
    SMART = "smart"
    PEEK = "peek"


def app_data_dir() -> Path:
    """Returns `%LocalAppData%\\Notch` and ensures it exists."""
    base = Path(os.environ.get("LOCALAPPDATA") or ".")
    directory = base / APP_DIR_NAME
    directory.mkdir(parents=True, exist_ok=True)
    return directory


@dataclass
class AppConfig:
    collapsed_width: float = 228.0
    collapsed_height: float = 34.0
    peek_width: float = 376.0
    peek_height: float = 64.0
    expanded_width: float = 540.0
    expanded_height: float = 296.0
    top_margin_px: float = 6.0
    hover_expand_delay_ms: int = 280
    hover_collapse_delay_ms: int = 360
    peek_duration_ms: int = 2600
    hide_in_fullscreen: bool = True
    visibility_mode: str = VisibilityMode.FIXED.value
    exclude_from_capture: bool = False
    discord_mute_hotkey: str = ""
    discord_deafen_hotkey: str = ""

    @classmethod
    def load(cls) -> AppConfig:
        path = app_data_dir() / CONFIG_FILE_NAME
        if not path.exists():
            cfg = cls()
            cfg.save()
            return cfg
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
            valid = {k: v for k, v in data.items() if k in cls.__dataclass_fields__}
            return cls(**valid)
        except Exception:
            return cls()

    def save(self) -> None:
        directory = app_data_dir()
        target = directory / CONFIG_FILE_NAME
        temp = directory / "config.json.tmp"
        temp.write_text(json.dumps(asdict(self), indent=2), encoding="utf-8")
        temp.replace(target)
