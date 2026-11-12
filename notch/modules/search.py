from __future__ import annotations

import urllib.parse

from notch.os_win32.winapi import SW_SHOWNORMAL, shell32


def build_google_search_url(query: str) -> str:
    return f"https://www.google.com/search?q={urllib.parse.quote_plus(query.strip())}"


def open_google_search(query: str) -> None:
    q = query.strip()
    if not q:
        return
    shell32.ShellExecuteW(None, "open", build_google_search_url(q), None, None, SW_SHOWNORMAL)
