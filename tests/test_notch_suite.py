from __future__ import annotations

import ctypes
import sqlite3
import tempfile
import time
import unittest
from ctypes import wintypes
from pathlib import Path

from notch.core.config import AppConfig
from notch.core.state_machine import ExpandedTab, NotchState, NotchStateMachine
from notch.core.storage import open_db
from notch.modules.audio import AudioController
from notch.modules.brightness import BrightnessController
from notch.modules.calendar_mod import CalendarManager
from notch.modules.discord_ipc import is_discord_running
from notch.modules.file_rack import FileRack
from notch.modules.launcher import ShortcutManager
from notch.modules.lyrics_lrclib import active_line_index, parse_lrc
from notch.modules.search import build_google_search_url
from notch.modules.tasks import TaskFilter, TaskManager, parse_quick_due
from notch.modules.timer import TimerState
from notch.os_win32.drag_drop import build_cf_hdrop_hglobal, extract_paths_from_hdrop
from notch.os_win32.winapi import (
    GWL_EXSTYLE,
    WS_EX_LAYERED,
    WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST,
    kernel32,
    user32,
)
from notch.os_win32.window import NotchWindow


class FILETIME(ctypes.Structure):
    _fields_ = [("dwLowDateTime", wintypes.DWORD), ("dwHighDateTime", wintypes.DWORD)]


class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


psapi = ctypes.WinDLL("psapi", use_last_error=True)
psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESS_MEMORY_COUNTERS), wintypes.DWORD]
psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
kernel32.GetCurrentProcess.restype = wintypes.HANDLE
kernel32.GetProcessTimes.argtypes = [
    wintypes.HANDLE,
    ctypes.POINTER(FILETIME),
    ctypes.POINTER(FILETIME),
    ctypes.POINTER(FILETIME),
    ctypes.POINTER(FILETIME),
]
kernel32.GetProcessTimes.restype = wintypes.BOOL
kernel32.GlobalFree.argtypes = [wintypes.HGLOBAL]


def _ft_to_100ns(ft: FILETIME) -> int:
    return (ft.dwHighDateTime << 32) | ft.dwLowDateTime


class NotchSuiteTests(unittest.TestCase):
    def test_state_machine_spring_settle_and_peek(self) -> None:
        cfg = AppConfig()
        t = 100.0
        sm = NotchStateMachine(cfg, now=t)
        self.assertEqual(sm.state, NotchState.COLLAPSED)

        sm.expand(ExpandedTab.MEDIA, cfg)
        self.assertEqual(sm.state, NotchState.EXPANDED)
        for _ in range(120):
            t += 0.016
            out = sm.tick(cfg, now=t)
            if not out.needs_timer:
                break
        self.assertAlmostEqual(sm.width.value, cfg.expanded_width, delta=1.0)
        self.assertAlmostEqual(sm.height.value, cfg.expanded_height, delta=1.0)

        sm.collapse(cfg)
        for _ in range(120):
            t += 0.016
            out = sm.tick(cfg, now=t)
            if not out.needs_timer:
                break
        self.assertEqual(sm.state, NotchState.COLLAPSED)
        self.assertAlmostEqual(sm.width.value, cfg.collapsed_width, delta=1.0)

        sm.trigger_peek("Song", "Artist", cfg, now=t)
        self.assertEqual(sm.state, NotchState.PEEK)
        t += (cfg.peek_duration_ms / 1000.0) + 0.1
        sm.tick(cfg, now=t)
        self.assertEqual(sm.state, NotchState.COLLAPSED)

    def test_modules_tasks_timer_calendar_shortcuts_lyrics_rack(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            conn = open_db(Path(td) / "test.db")
            try:
                tm = TaskManager(conn)
                title, due = parse_quick_due("Buy milk 14:30")
                self.assertEqual(title, "Buy milk")
                self.assertIsNotNone(due)
                item = tm.add_task(title, due_at=1000.0)
                self.assertEqual(len(tm.list_tasks(TaskFilter.TODAY)), 1)
                due_list = tm.pop_due_reminders(1005.0)
                self.assertEqual(len(due_list), 1)
                tm.toggle_done(item.id)
                self.assertEqual(len(tm.list_tasks(TaskFilter.COMPLETED)), 1)
                tm.delete_task(item.id)
                self.assertEqual(len(tm.list_tasks(TaskFilter.COMPLETED)), 0)

                timer = TimerState()
                timer.set_preset(2, "Quick")
                timer.toggle_start_pause(now=10.0)
                self.assertFalse(timer.tick(now=11.0))
                self.assertTrue(timer.tick(now=12.5))
                self.assertEqual(timer.formatted_remaining(), "00:00")

                ics = Path(td) / "test.ics"
                ics.write_text(
                    "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:Standup\nDTSTART:20261112T090000Z\nEND:VEVENT\nEND:VCALENDAR\n",
                    encoding="utf-8",
                )
                cal = CalendarManager(tm)
                self.assertEqual(cal.import_ics_file(ics), 1)
                self.assertGreaterEqual(len(cal.get_agenda()), 1)

                sample = Path(td) / "notes.txt"
                sample.write_text("hello", encoding="utf-8")
                rack = FileRack()
                self.assertEqual(rack.add_paths([str(sample)]), 1)
                self.assertEqual(len(rack.list_items()), 1)
                rack.remove_path(str(sample))
                self.assertTrue(sample.exists())  # reference-only, never moves/deletes user files

                hdrop = build_cf_hdrop_hglobal([str(sample)])
                try:
                    extracted = extract_paths_from_hdrop(hdrop)
                    self.assertEqual(extracted, [str(sample)])
                finally:
                    kernel32.GlobalFree(hdrop)

                sm_short = ShortcutManager(conn)
                sc = sm_short.pin_shortcut("https://example.com", "Example")
                self.assertEqual(len(sm_short.list_shortcuts()), 1)
                sm_short.remove_shortcut(sc.id)
                self.assertEqual(len(sm_short.list_shortcuts()), 0)
            finally:
                conn.close()

        lines = parse_lrc("[00:10.00]Line one\n[00:20.50]Line two\n")
        self.assertEqual(len(lines), 2)
        self.assertEqual(active_line_index(lines, 15.0), 0)
        self.assertEqual(active_line_index(lines, 21.0), 1)
        self.assertEqual(build_google_search_url("rust notch"), "https://www.google.com/search?q=rust+notch")
        self.assertIsInstance(is_discord_running(), bool)

        audio = AudioController()
        vol, muted = audio.get_master_volume()
        self.assertTrue(0.0 <= vol <= 1.0)
        self.assertIsInstance(muted, bool)
        bright = BrightnessController().get_brightness()
        self.assertTrue(0.0 <= bright.level <= 1.0)

    def test_live_window_styles_focus_tabs_pressed_and_idle_footprint(self) -> None:
        win = NotchWindow(enable_tray=False, enable_hotkeys=False)
        try:
            self.assertIsNotNone(win.hwnd)
            ex = user32.GetWindowLongPtrW(win.hwnd, GWL_EXSTYLE)
            self.assertTrue(ex & WS_EX_LAYERED)
            self.assertTrue(ex & WS_EX_TOOLWINDOW)
            self.assertTrue(ex & WS_EX_TOPMOST)
            self.assertTrue(ex & WS_EX_NOACTIVATE)

            # Smart focus handoff clears WS_EX_NOACTIVATE, then restores it
            win.set_smart_focus(True, "google_search")
            ex_focused = user32.GetWindowLongPtrW(win.hwnd, GWL_EXSTYLE)
            self.assertFalse(ex_focused & WS_EX_NOACTIVATE)
            win.set_smart_focus(False)
            ex_restored = user32.GetWindowLongPtrW(win.hwnd, GWL_EXSTYLE)
            self.assertTrue(ex_restored & WS_EX_NOACTIVATE)

            # Render all 8 ExpandedTab panels + PRESSED state
            for tab in ExpandedTab:
                win.sm.expand(tab, win.config)
                win.sm.width.value = win.config.expanded_width
                win.sm.height.value = win.config.expanded_height
                win.ui_state.interaction.pressed_id = "pill_collapse"
                win.paint()
            win.ui_state.interaction.pressed_id = None

            # Collapse and settle, then measure idle CPU & Working Set RAM
            win.sm.collapse(win.config)
            win.sm.width.value = win.config.collapsed_width
            win.sm.height.value = win.config.collapsed_height
            win._set_anim_timer(False)
            win.paint()

            proc = kernel32.GetCurrentProcess()
            c0, e0, k0, u0 = FILETIME(), FILETIME(), FILETIME(), FILETIME()
            c1, e1, k1, u1 = FILETIME(), FILETIME(), FILETIME(), FILETIME()
            kernel32.GetProcessTimes(proc, ctypes.byref(c0), ctypes.byref(e0), ctypes.byref(k0), ctypes.byref(u0))
            time.sleep(0.5)
            kernel32.GetProcessTimes(proc, ctypes.byref(c1), ctypes.byref(e1), ctypes.byref(k1), ctypes.byref(u1))
            cpu_secs = ((_ft_to_100ns(k1) - _ft_to_100ns(k0)) + (_ft_to_100ns(u1) - _ft_to_100ns(u0))) / 1e7
            idle_cpu_pct = (cpu_secs / 0.5) * 100.0

            pmc = PROCESS_MEMORY_COUNTERS()
            pmc.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS)
            psapi.GetProcessMemoryInfo(proc, ctypes.byref(pmc), pmc.cb)
            ram_mb = pmc.WorkingSetSize / (1024.0 * 1024.0)

            print(f"\n[AUDIT] Settled Idle CPU: {idle_cpu_pct:.2f}% | Working Set RAM: {ram_mb:.1f} MB")
            self.assertLess(idle_cpu_pct, 2.0)
            self.assertLess(ram_mb, 80.0)
        finally:
            win.destroy()


if __name__ == "__main__":
    unittest.main()
