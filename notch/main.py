from __future__ import annotations

from notch.os_win32.window import NotchWindow


def main() -> int:
    app = NotchWindow()
    app.run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
