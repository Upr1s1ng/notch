from __future__ import annotations

import ctypes
import time
from ctypes import wintypes
from pathlib import Path

from notch.core.config import AppConfig, VisibilityMode
from notch.core.state_machine import ExpandedTab, NotchState, NotchStateMachine
from notch.core.storage import open_db
from notch.modules.audio import AudioController
from notch.modules.brightness import BrightnessController
from notch.modules.calendar_mod import CalendarManager
from notch.modules.discord_ipc import DiscordController, DiscordVoiceState
from notch.modules.file_rack import FileRack
from notch.modules.launcher import ShortcutManager
from notch.modules.lyrics_lrclib import LyricsResult, LyricsService, active_line_index
from notch.modules.media_smtc import SmtcController
from notch.modules.search import open_google_search
from notch.modules.tasks import TaskFilter, TaskManager, parse_quick_due
from notch.modules.timer import TimerState
from notch.os_win32.drag_drop import OleDropTarget, ensure_ole_initialized, start_cf_hdrop_drag
from notch.os_win32.winapi import (
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    EVENT_SYSTEM_FOREGROUND,
    GWL_EXSTYLE,
    HWND_TOPMOST,
    MA_ACTIVATE,
    MA_NOACTIVATE,
    MF_CHECKED,
    MF_SEPARATOR,
    MF_STRING,
    MF_UNCHECKED,
    MOD_ALT,
    MOD_CONTROL,
    MONITOR_DEFAULTTONEAREST,
    MONITORINFO,
    NIF_ICON,
    NIF_MESSAGE,
    NIF_TIP,
    NIM_ADD,
    NIM_DELETE,
    NOTIFYICONDATAW,
    QUNS_BUSY,
    QUNS_RUNNING_D3D_FULL_SCREEN,
    SWP_FRAMECHANGED,
    SWP_NOACTIVATE,
    SWP_NOMOVE,
    SWP_NOSIZE,
    SWP_NOZORDER,
    SW_HIDE,
    SW_SHOWNOACTIVATE,
    TME_LEAVE,
    TPM_BOTTOMALIGN,
    TPM_LEFTALIGN,
    TRACKMOUSEEVENT,
    VK_BACK,
    VK_DELETE,
    VK_ESCAPE,
    VK_LEFT,
    VK_RETURN,
    VK_RIGHT,
    VK_SPACE,
    WA_INACTIVE,
    WDA_EXCLUDEFROMCAPTURE,
    WDA_NONE,
    WINEVENTPROC,
    WINEVENT_OUTOFCONTEXT,
    WM_ACTIVATE,
    WM_APP,
    WM_CHAR,
    WM_CLOSE,
    WM_COMMAND,
    WM_DESTROY,
    WM_DPICHANGED,
    WM_HOTKEY,
    WM_KEYDOWN,
    WM_KILLFOCUS,
    WM_LBUTTONDOWN,
    WM_LBUTTONUP,
    WM_MOUSEACTIVATE,
    WM_MOUSELEAVE,
    WM_MOUSEMOVE,
    WM_RBUTTONUP,
    WM_TIMER,
    WNDCLASSEXW,
    WNDPROC,
    WS_EX_LAYERED,
    WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST,
    WS_POPUP,
    kernel32,
    shell32,
    user32,
    winmm,
)
from notch.ui.gdiplus import LayeredCanvas
from notch.ui.media_audio_panels import MediaAudioCallbacks
from notch.ui.productivity_panels import ProductivityCallbacks
from notch.ui.rack_panel import RackCallbacks
from notch.ui.shell import ShellCallbacks, ShellUiState, draw_notch_shell

ole32 = ctypes.WinDLL("ole32", use_last_error=True)
ole32.RegisterDragDrop.argtypes = [wintypes.HWND, ctypes.c_void_p]
ole32.RegisterDragDrop.restype = ctypes.c_long
ole32.RevokeDragDrop.argtypes = [wintypes.HWND]
ole32.RevokeDragDrop.restype = ctypes.c_long

comdlg32 = ctypes.WinDLL("comdlg32", use_last_error=True)


class OPENFILENAMEW(ctypes.Structure):
    _fields_ = [
        ("lStructSize", wintypes.DWORD),
        ("hwndOwner", wintypes.HWND),
        ("hInstance", wintypes.HINSTANCE),
        ("lpstrFilter", wintypes.LPCWSTR),
        ("lpstrCustomFilter", wintypes.LPWSTR),
        ("nMaxCustFilter", wintypes.DWORD),
        ("nFilterIndex", wintypes.DWORD),
        ("lpstrFile", wintypes.LPWSTR),
        ("nMaxFile", wintypes.DWORD),
        ("lpstrFileTitle", wintypes.LPWSTR),
        ("nMaxFileTitle", wintypes.DWORD),
        ("lpstrInitialDir", wintypes.LPCWSTR),
        ("lpstrTitle", wintypes.LPCWSTR),
        ("Flags", wintypes.DWORD),
        ("nFileOffset", wintypes.WORD),
        ("nFileExtension", wintypes.WORD),
        ("lpstrDefExt", wintypes.LPCWSTR),
        ("lCustData", ctypes.c_size_t),
        ("lpfnHook", ctypes.c_void_p),
        ("lpTemplateName", wintypes.LPCWSTR),
        ("pvReserved", ctypes.c_void_p),
        ("dwReserved", wintypes.DWORD),
        ("FlagsEx", wintypes.DWORD),
    ]


comdlg32.GetOpenFileNameW.argtypes = [ctypes.POINTER(OPENFILENAMEW)]
comdlg32.GetOpenFileNameW.restype = wintypes.BOOL

kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
kernel32.GetModuleHandleW.restype = wintypes.HMODULE

TIMER_ANIM_ID = 1
TIMER_SLOW_ID = 2
HOTKEY_TOGGLE_HIDE = 1
HOTKEY_SEARCH = 2

WM_TRAY = WM_APP + 1
WM_APP_REFRESH = WM_APP + 2

IDM_TOGGLE = 1001
IDM_MODE_FIXED = 1002
IDM_MODE_SMART = 1003
IDM_MODE_PEEK = 1004
IDM_PRIVACY = 1005
IDM_EXIT = 1006

CLASS_NAME = "NotchDynamicIslandClass"
_HWND_INSTANCES: dict[int, NotchWindow] = {}
_CLASS_REGISTERED = False


def _global_wndproc(hwnd: wintypes.HWND, msg: int, wparam: int, lparam: int) -> int:
    inst = _HWND_INSTANCES.get(int(hwnd))
    if inst is not None:
        return inst._wndproc(hwnd, msg, wparam, lparam)
    return user32.DefWindowProcW(hwnd, msg, wparam, lparam)


_GLOBAL_WNDPROC = WNDPROC(_global_wndproc)


def _signed_xy(lparam: int) -> tuple[int, int]:
    x = ctypes.c_short(lparam & 0xFFFF).value
    y = ctypes.c_short((lparam >> 16) & 0xFFFF).value
    return (x, y)


def _pick_file(hwnd: wintypes.HWND, title: str, filter_str: str) -> str | None:
    buf = ctypes.create_unicode_buffer(1024)
    ofn = OPENFILENAMEW()
    ofn.lStructSize = ctypes.sizeof(OPENFILENAMEW)
    ofn.hwndOwner = hwnd
    ofn.lpstrFilter = filter_str
    ofn.lpstrFile = ctypes.cast(buf, wintypes.LPWSTR)
    ofn.nMaxFile = len(buf)
    ofn.lpstrTitle = title
    ofn.Flags = 0x00080000 | 0x00001000 | 0x00000800  # EXPLORER | FILEMUSTEXIST | PATHMUSTEXIST
    if comdlg32.GetOpenFileNameW(ctypes.byref(ofn)):
        return buf.value
    return None


class NotchWindow:
    def __init__(self, config: AppConfig | None = None, enable_tray: bool = True, enable_hotkeys: bool = True) -> None:
        user32.SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        ensure_ole_initialized()

        self.config = config or AppConfig.load()
        self.sm = NotchStateMachine(self.config)
        self.ui_state = ShellUiState(capture_excluded=self.config.exclude_from_capture)
        self.canvas = LayeredCanvas()

        self.conn = open_db()
        self.audio = AudioController()
        self.brightness = BrightnessController()
        self.smtc = SmtcController()
        self.lyrics = LyricsService()
        self.tasks = TaskManager(self.conn)
        self.calendar = CalendarManager(self.tasks)
        self.shortcuts = ShortcutManager(self.conn)
        self.timer = TimerState()
        self.file_rack = FileRack()
        self.discord = DiscordController(self.config, self.audio, self._on_discord_state)

        self._enable_tray = enable_tray
        self._enable_hotkeys = enable_hotkeys
        self._tray_added = False
        self._anim_timer_active = False
        self._smart_focus_active = False
        self._tracking_mouse = False
        self._last_lyric_track_key = ""
        self._last_lyrics_result = LyricsResult(track_key="")
        self._win_event_hook: wintypes.HANDLE | None = None
        self._win_event_proc = WINEVENTPROC(self._on_win_event)
        self._drop_target: OleDropTarget | None = None
        self._drop_registered = False
        self._last_mon_rect = (0, 0, 1920, 1080)
        self._dpi_scale = 1.0
        self.hwnd: wintypes.HWND | None = None

        self.callbacks = self._build_callbacks()
        self._sync_data_to_ui(refresh_heavy=True)
        self._create_window()
        self.discord.start()

    def _build_callbacks(self) -> ShellCallbacks:
        media_audio = MediaAudioCallbacks(
            on_toggle_play=self._on_toggle_play,
            on_next_track=self._on_next_track,
            on_prev_track=self._on_prev_track,
            on_seek=self._on_seek,
            on_cycle_source=self._on_cycle_source,
            on_set_master_volume=self._on_set_master_volume,
            on_toggle_master_mute=self._on_toggle_master_mute,
            on_set_brightness=self._on_set_brightness,
            on_set_app_volume=self._on_set_app_volume,
            on_toggle_app_mute=self._on_toggle_app_mute,
        )
        productivity = ProductivityCallbacks(
            on_set_task_filter=self._on_set_task_filter,
            on_add_task=self._on_add_task,
            on_toggle_task=self._on_toggle_task,
            on_delete_task=self._on_delete_task,
            on_move_task=self._on_move_task,
            on_import_ics=self._on_import_ics,
            on_timer_toggle=self._on_timer_toggle,
            on_timer_reset=self._on_timer_reset,
            on_timer_preset=self._on_timer_preset,
            on_launch_shortcut=self._on_launch_shortcut,
            on_remove_shortcut=self._on_remove_shortcut,
            on_pin_file_dialog=self._on_pin_file_dialog,
            on_pin_text_shortcut=self._on_pin_text_shortcut,
        )
        rack = RackCallbacks(
            on_add_file_dialog=self._on_add_rack_file_dialog,
            on_open_item=self._on_open_rack_item,
            on_remove_item=self._on_remove_rack_item,
            on_clear=self._on_clear_rack,
        )
        return ShellCallbacks(
            on_toggle_expand=self._on_toggle_expand,
            on_collapse=self._on_collapse,
            on_hide_manual=self._on_hide_manual,
            on_trigger_peek=self._on_trigger_peek_demo,
            on_submit_search=self._on_submit_search,
            on_select_tab=self._on_select_tab,
            on_discord_mute=self.discord.toggle_mute,
            on_discord_deafen=self.discord.toggle_deafen,
            on_discord_disconnect=self.discord.disconnect_call,
            on_discord_toggle_hotkey_preset=self._on_toggle_discord_hotkey_preset,
            media_audio=media_audio,
            productivity=productivity,
            rack=rack,
            on_toggle_capture_privacy=self.toggle_capture_privacy,
        )

    def _create_window(self) -> None:
        global _CLASS_REGISTERED
        hinst = kernel32.GetModuleHandleW(None)
        if not _CLASS_REGISTERED:
            wc = WNDCLASSEXW()
            wc.cbSize = ctypes.sizeof(WNDCLASSEXW)
            wc.lpfnWndProc = _GLOBAL_WNDPROC
            wc.hInstance = hinst
            wc.hIcon = user32.LoadIconW(None, ctypes.c_void_p(32512))
            wc.hCursor = user32.LoadCursorW(None, ctypes.c_void_p(32512))
            wc.lpszClassName = CLASS_NAME
            user32.RegisterClassExW(ctypes.byref(wc))
            _CLASS_REGISTERED = True

        ex_style = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE
        hwnd = user32.CreateWindowExW(
            ex_style, CLASS_NAME, "Notch", WS_POPUP,
            0, 0, 640, 320, None, None, hinst, None,
        )
        if not hwnd:
            raise OSError("Failed to create Notch window")
        self.hwnd = hwnd
        _HWND_INSTANCES[int(hwnd)] = self

        dpi = user32.GetDpiForWindow(hwnd) or 96
        self._dpi_scale = max(1.0, dpi / 96.0)
        self._update_monitor_bounds()

        if self.config.exclude_from_capture:
            user32.SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)

        if self._enable_hotkeys:
            user32.RegisterHotKey(hwnd, HOTKEY_TOGGLE_HIDE, MOD_CONTROL | MOD_ALT, ord("D"))
            user32.RegisterHotKey(hwnd, HOTKEY_SEARCH, MOD_CONTROL | MOD_ALT, VK_SPACE)

        if self._enable_tray:
            self._add_tray_icon()

        self._win_event_hook = user32.SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None,
            self._win_event_proc, 0, 0, WINEVENT_OUTOFCONTEXT,
        )

        self._drop_target = OleDropTarget(on_drag_enter=self._on_ole_drag_enter, on_drop_paths=self._on_ole_drop_paths)
        if ole32.RegisterDragDrop(hwnd, self._drop_target.com_ptr) == 0:
            self._drop_registered = True

        user32.SetTimer(hwnd, TIMER_SLOW_ID, 1000, None)
        user32.ShowWindow(hwnd, SW_SHOWNOACTIVATE)
        self._check_fullscreen_or_visibility()
        self.paint()

    def _window_pixel_size(self) -> tuple[int, int]:
        w = round((self.config.expanded_width + 48.0) * self._dpi_scale)
        h = round((self.config.expanded_height + 44.0) * self._dpi_scale)
        return (w, h)

    def _update_monitor_bounds(self) -> None:
        pt = wintypes.POINT(0, 0)
        hmon = user32.MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
        mi = MONITORINFO()
        mi.cbSize = ctypes.sizeof(MONITORINFO)
        if hmon and user32.GetMonitorInfoW(hmon, ctypes.byref(mi)):
            r = mi.rcMonitor
            self._last_mon_rect = (r.left, r.top, r.right, r.bottom)

    def _window_origin(self) -> tuple[int, int]:
        left, top, right, _bottom = self._last_mon_rect
        win_w, _win_h = self._window_pixel_size()
        return (left + (right - left - win_w) // 2, top)

    def _set_anim_timer(self, active: bool) -> None:
        if not self.hwnd or active == self._anim_timer_active:
            return
        self._anim_timer_active = active
        if active:
            winmm.timeBeginPeriod(1)
            user32.SetTimer(self.hwnd, TIMER_ANIM_ID, 8, None)
        else:
            user32.KillTimer(self.hwnd, TIMER_ANIM_ID)
            winmm.timeEndPeriod(1)

    def paint(self) -> None:
        if not self.hwnd or self.sm.state == NotchState.HIDDEN:
            return
        win_w, win_h = self._window_pixel_size()
        x, y = self._window_origin()
        self.canvas.render_and_present(
            self.hwnd, x, y, win_w, win_h,
            lambda painter: draw_notch_shell(
                painter, self.sm, self.ui_state, self.config,
                self._dpi_scale, win_w, win_h, self.callbacks,
            ),
        )

    def set_smart_focus(self, enabled: bool, input_id: str | None = None) -> None:
        if not self.hwnd:
            return
        self._smart_focus_active = enabled
        ex = user32.GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE)
        if enabled:
            self.sm.input_focused = True
            self.ui_state.set_focused_input(input_id)
            user32.SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex & ~WS_EX_NOACTIVATE)
            user32.SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED)
            user32.SetForegroundWindow(self.hwnd)
        else:
            self.sm.input_focused = False
            self.ui_state.set_focused_input(None)
            user32.SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex | WS_EX_NOACTIVATE)
            user32.SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED)
        self.paint()

    def toggle_capture_privacy(self) -> None:
        self.config.exclude_from_capture = not self.config.exclude_from_capture
        self.ui_state.capture_excluded = self.config.exclude_from_capture
        if self.hwnd:
            user32.SetWindowDisplayAffinity(
                self.hwnd,
                WDA_EXCLUDEFROMCAPTURE if self.config.exclude_from_capture else WDA_NONE,
            )
        self.config.save()
        self.paint()

    def _set_visibility_mode(self, mode: VisibilityMode) -> None:
        self.config.visibility_mode = mode
        self.config.save()
        self._check_fullscreen_or_visibility()

    def _on_win_event(self, _hook, _event, _hwnd, _id_obj, _id_child, _thread, _time) -> None:
        self._check_fullscreen_or_visibility()

    def _check_fullscreen_or_visibility(self) -> None:
        if not self.hwnd:
            return
        state = ctypes.c_int(0)
        is_fs = False
        if shell32.SHQueryUserNotificationState(ctypes.byref(state)) == 0:
            is_fs = state.value in (QUNS_BUSY, QUNS_RUNNING_D3D_FULL_SCREEN)
        if not is_fs:
            fg = user32.GetForegroundWindow()
            if fg and int(fg) != int(self.hwnd):
                rect = wintypes.RECT()
                if user32.GetWindowRect(fg, ctypes.byref(rect)):
                    l, t, r, b = self._last_mon_rect
                    if rect.left <= l and rect.top <= t and rect.right >= r and rect.bottom >= b:
                        is_fs = True
        was_hidden = self.sm.state == NotchState.HIDDEN
        self.sm.set_fullscreen_hidden(is_fs, self.config)
        self._sync_window_visibility(was_hidden)

    def _sync_window_visibility(self, was_hidden: bool) -> None:
        if not self.hwnd:
            return
        is_hidden = self.sm.state == NotchState.HIDDEN
        if is_hidden and not was_hidden:
            self._set_anim_timer(False)
            user32.ShowWindow(self.hwnd, SW_HIDE)
        elif not is_hidden and was_hidden:
            user32.ShowWindow(self.hwnd, SW_SHOWNOACTIVATE)
            self.paint()

    def _tick_animation(self) -> None:
        out = self.sm.tick(self.config)
        self._set_anim_timer(out.needs_timer)
        if out.needs_repaint:
            self.paint()

    def _sync_data_to_ui(self, refresh_heavy: bool = False) -> None:
        self.ui_state.productivity.tasks = self.tasks.list_tasks(self.ui_state.productivity.task_filter)
        self.ui_state.productivity.agenda = self.calendar.get_agenda()
        self.ui_state.productivity.shortcuts = self.shortcuts.list_shortcuts()
        self.ui_state.productivity.timer = self.timer
        self.ui_state.rack.items = self.file_rack.list_items()

        if refresh_heavy:
            vol, muted = self.audio.get_master_volume()
            self.ui_state.media_audio.master_volume = vol
            self.ui_state.media_audio.master_muted = muted
            self.ui_state.media_audio.app_sessions = self.audio.list_app_sessions()
            b = self.brightness.get_brightness(self.hwnd)
            self.ui_state.media_audio.brightness_level = b.level
            self.ui_state.media_audio.brightness_supported = b.supported
            snap = self.smtc.read_snapshot()
            self.ui_state.media_audio.media = snap
            if snap.has_session:
                track_key = f"{snap.title}|{snap.artist}|{snap.album}"
                if track_key != self._last_lyric_track_key:
                    self._last_lyric_track_key = track_key
                    self.lyrics.fetch_async(snap.title, snap.artist, snap.album, snap.duration_secs, self._on_lyrics_ready)
            if self._last_lyrics_result.synced:
                idx = active_line_index(self._last_lyrics_result.synced, snap.live_position_secs())
                self.ui_state.media_audio.lyric_line = self._last_lyrics_result.synced[idx].text if idx >= 0 else ""
            else:
                self.ui_state.media_audio.lyric_line = self._last_lyrics_result.plain[0] if self._last_lyrics_result.plain else ""

    def _on_lyrics_ready(self, res: LyricsResult) -> None:
        self._last_lyrics_result = res
        if res.synced:
            pos = self.ui_state.media_audio.media.live_position_secs()
            idx = active_line_index(res.synced, pos)
            self.ui_state.media_audio.lyric_line = res.synced[idx].text if idx >= 0 else ""
        else:
            self.ui_state.media_audio.lyric_line = res.plain[0] if res.plain else ""
        if self.hwnd:
            user32.PostMessageW(self.hwnd, WM_APP_REFRESH, 0, 0)

    def _on_discord_state(self, st: DiscordVoiceState) -> None:
        self.ui_state.discord = st
        if self.hwnd:
            user32.PostMessageW(self.hwnd, WM_APP_REFRESH, 0, 0)

    # --- Shell / Module Callbacks ---
    def _on_toggle_expand(self) -> None:
        self.sm.toggle_expand(self.config)
        self._sync_data_to_ui(refresh_heavy=True)
        self._tick_animation()

    def _on_collapse(self) -> None:
        self.set_smart_focus(False)
        self.sm.collapse(self.config)
        self._tick_animation()

    def _on_hide_manual(self) -> None:
        was_hidden = self.sm.state == NotchState.HIDDEN
        self.sm.toggle_manual_hide(self.config)
        self._sync_window_visibility(was_hidden)

    def _on_trigger_peek_demo(self) -> None:
        self.sm.trigger_peek("Notch Ready", "Hover or click to expand modules", self.config)
        self._tick_animation()

    def _on_submit_search(self) -> None:
        q = self.ui_state.search_input.text.strip()
        if q:
            open_google_search(q)
            self.ui_state.search_input.clear()
        self.set_smart_focus(False)

    def _on_select_tab(self, tab: ExpandedTab) -> None:
        self.set_smart_focus(False)
        self.sm.expand(tab, self.config)
        if tab in (ExpandedTab.MEDIA, ExpandedTab.CONTROLS, ExpandedTab.OVERVIEW):
            self._sync_data_to_ui(refresh_heavy=True)
        else:
            self._sync_data_to_ui(refresh_heavy=False)
        self._tick_animation()

    def _on_toggle_discord_hotkey_preset(self) -> None:
        self.config.discord_mute_hotkey = "Ctrl+Shift+M"
        self.config.save()
        self.paint()

    def _on_toggle_play(self) -> None:
        self.smtc.toggle_play_pause()

    def _on_next_track(self) -> None:
        self.smtc.next_track()

    def _on_prev_track(self) -> None:
        self.smtc.prev_track()

    def _on_seek(self, ratio: float) -> None:
        dur = self.ui_state.media_audio.media.duration_secs
        if dur > 0:
            self.smtc.seek_to_seconds(dur * ratio)

    def _on_cycle_source(self) -> None:
        self.smtc.cycle_source()

    def _on_set_master_volume(self, ratio: float) -> None:
        self.audio.set_master_volume(ratio)
        self.ui_state.media_audio.master_volume = max(0.0, min(1.0, ratio))
        self.paint()

    def _on_toggle_master_mute(self) -> None:
        self.ui_state.media_audio.master_muted = self.audio.toggle_master_mute()
        self.paint()

    def _on_set_brightness(self, ratio: float) -> None:
        self.brightness.set_brightness(ratio, self.hwnd)
        self.ui_state.media_audio.brightness_level = max(0.0, min(1.0, ratio))
        self.paint()

    def _on_set_app_volume(self, pid: int, ratio: float) -> None:
        self.audio.set_app_volume(pid, ratio)
        for s in self.ui_state.media_audio.app_sessions:
            if s.pid == pid:
                s.volume = max(0.0, min(1.0, ratio))
        self.paint()

    def _on_toggle_app_mute(self, pid: int, mute: bool) -> None:
        self.audio.toggle_app_mute(pid, mute)
        for s in self.ui_state.media_audio.app_sessions:
            if s.pid == pid:
                s.muted = mute
        self.paint()

    def _on_set_task_filter(self, flt: TaskFilter) -> None:
        self.ui_state.productivity.task_filter = flt
        self._sync_data_to_ui()
        self.paint()

    def _on_add_task(self) -> None:
        raw = self.ui_state.productivity.task_input.text.strip()
        if raw:
            title, due_at = parse_quick_due(raw)
            self.tasks.add_task(title, due_at)
            self.ui_state.productivity.task_input.clear()
            self._sync_data_to_ui()
        self.set_smart_focus(False)

    def _on_toggle_task(self, task_id: int) -> None:
        self.tasks.toggle_done(task_id)
        self._sync_data_to_ui()
        self.paint()

    def _on_delete_task(self, task_id: int) -> None:
        self.tasks.delete_task(task_id)
        self._sync_data_to_ui()
        self.paint()

    def _on_move_task(self, task_id: int, direction: int) -> None:
        self.tasks.move_task(task_id, direction)
        self._sync_data_to_ui()
        self.paint()

    def _on_import_ics(self) -> None:
        if not self.hwnd:
            return
        picked = _pick_file(self.hwnd, "Import Calendar (.ics)", "iCalendar Files (*.ics)\0*.ics\0All Files (*.*)\0*.*\0\0")
        if picked:
            self.calendar.import_ics_file(Path(picked))
            self._sync_data_to_ui()
            self.paint()

    def _on_timer_toggle(self) -> None:
        self.timer.toggle_start_pause()
        self.paint()

    def _on_timer_reset(self) -> None:
        self.timer.reset()
        self.paint()

    def _on_timer_preset(self, secs: int, label: str) -> None:
        self.timer.set_preset(secs, label)
        self.paint()

    def _on_launch_shortcut(self, target: str) -> None:
        self.shortcuts.launch(target)

    def _on_remove_shortcut(self, shortcut_id: int) -> None:
        self.shortcuts.remove_shortcut(shortcut_id)
        self._sync_data_to_ui()
        self.paint()

    def _on_pin_file_dialog(self) -> None:
        if not self.hwnd:
            return
        picked = _pick_file(self.hwnd, "Pin Application or File", "Executables & All Files\0*.exe;*.lnk;*.bat;*.*\0All Files (*.*)\0*.*\0\0")
        if picked:
            self.shortcuts.pin_shortcut(picked)
            self._sync_data_to_ui()
            self.paint()

    def _on_pin_text_shortcut(self) -> None:
        raw = self.ui_state.productivity.shortcut_input.text.strip()
        if raw:
            self.shortcuts.pin_shortcut(raw)
            self.ui_state.productivity.shortcut_input.clear()
            self._sync_data_to_ui()
        self.set_smart_focus(False)

    def _on_add_rack_file_dialog(self) -> None:
        if not self.hwnd:
            return
        picked = _pick_file(self.hwnd, "Pin File Reference to Rack", "All Files (*.*)\0*.*\0\0")
        if picked:
            self.file_rack.add_paths([picked])
            self._sync_data_to_ui()
            self.paint()

    def _on_open_rack_item(self, path: str) -> None:
        self.file_rack.open_item(path)

    def _on_remove_rack_item(self, path: str) -> None:
        self.file_rack.remove_path(path)
        self._sync_data_to_ui()
        self.paint()

    def _on_clear_rack(self) -> None:
        self.file_rack.clear()
        self._sync_data_to_ui()
        self.paint()

    def _on_ole_drag_enter(self) -> None:
        if self.sm.state != NotchState.HIDDEN:
            self.sm.expand(ExpandedTab.RACK, self.config)
            self._tick_animation()

    def _on_ole_drop_paths(self, paths: list[str]) -> None:
        if self.file_rack.add_paths(paths):
            self._sync_data_to_ui()
            self.sm.expand(ExpandedTab.RACK, self.config)
            self._tick_animation()

    # --- Tray Icon ---
    def _add_tray_icon(self) -> None:
        if not self.hwnd or self._tray_added:
            return
        nid = NOTIFYICONDATAW()
        nid.cbSize = ctypes.sizeof(NOTIFYICONDATAW)
        nid.hWnd = self.hwnd
        nid.uID = 1
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP
        nid.uCallbackMessage = WM_TRAY
        nid.hIcon = user32.LoadIconW(None, ctypes.c_void_p(32512))
        nid.szTip = "Notch — Dynamic Island"
        shell32.Shell_NotifyIconW(NIM_ADD, ctypes.byref(nid))
        self._tray_added = True

    def _remove_tray_icon(self) -> None:
        if not self.hwnd or not self._tray_added:
            return
        nid = NOTIFYICONDATAW()
        nid.cbSize = ctypes.sizeof(NOTIFYICONDATAW)
        nid.hWnd = self.hwnd
        nid.uID = 1
        shell32.Shell_NotifyIconW(NIM_DELETE, ctypes.byref(nid))
        self._tray_added = False

    def _show_tray_menu(self) -> None:
        if not self.hwnd:
            return
        menu = user32.CreatePopupMenu()
        if not menu:
            return
        user32.AppendMenuW(menu, MF_STRING, IDM_TOGGLE, "Expand / Collapse Notch")
        user32.AppendMenuW(menu, MF_SEPARATOR, 0, None)
        mode = self.config.visibility_mode
        user32.AppendMenuW(menu, MF_STRING | (MF_CHECKED if mode == VisibilityMode.FIXED else MF_UNCHECKED), IDM_MODE_FIXED, "Mode: Fixed")
        user32.AppendMenuW(menu, MF_STRING | (MF_CHECKED if mode == VisibilityMode.SMART else MF_UNCHECKED), IDM_MODE_SMART, "Mode: Smart")
        user32.AppendMenuW(menu, MF_STRING | (MF_CHECKED if mode == VisibilityMode.PEEK else MF_UNCHECKED), IDM_MODE_PEEK, "Mode: Peek")
        user32.AppendMenuW(menu, MF_SEPARATOR, 0, None)
        user32.AppendMenuW(menu, MF_STRING | (MF_CHECKED if self.config.exclude_from_capture else MF_UNCHECKED), IDM_PRIVACY, "Hide from Screen Capture")
        user32.AppendMenuW(menu, MF_SEPARATOR, 0, None)
        user32.AppendMenuW(menu, MF_STRING, IDM_EXIT, "Exit Notch")
        pt = wintypes.POINT()
        user32.GetCursorPos(ctypes.byref(pt))
        user32.SetForegroundWindow(self.hwnd)
        user32.TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, 0, self.hwnd, None)
        user32.DestroyMenu(menu)

    # --- WndProc ---
    def _wndproc(self, hwnd: wintypes.HWND, msg: int, wparam: int, lparam: int) -> int:
        if msg == WM_MOUSEACTIVATE:
            return MA_ACTIVATE if self._smart_focus_active else MA_NOACTIVATE

        if msg == WM_ACTIVATE and (wparam & 0xFFFF) == WA_INACTIVE and self._smart_focus_active:
            self.set_smart_focus(False)
            return 0

        if msg == WM_KILLFOCUS and self._smart_focus_active:
            self.set_smart_focus(False)
            return 0

        if msg == WM_MOUSEMOVE:
            if not self._tracking_mouse:
                tme = TRACKMOUSEEVENT(cbSize=ctypes.sizeof(TRACKMOUSEEVENT), dwFlags=TME_LEAVE, hwndTrack=hwnd, dwHoverTime=0)
                user32.TrackMouseEvent(ctypes.byref(tme))
                self._tracking_mouse = True
            px, py = _signed_xy(lparam)
            lx, ly = px / self._dpi_scale, py / self._dpi_scale
            drag_paths = self.ui_state.interaction.consume_drag_start(lx, ly)
            if drag_paths:
                self.ui_state.interaction.pressed_id = None
                start_cf_hdrop_drag(drag_paths)
                return 0
            self.sm.on_hover_change(True, self.config)
            hover_changed = self.ui_state.interaction.update_hover(lx, ly)
            if hover_changed or self.ui_state.interaction.active_slider_id is not None:
                self.paint()
            self._tick_animation()
            return 0

        if msg == WM_MOUSELEAVE:
            self._tracking_mouse = False
            self.ui_state.interaction.hovered_id = None
            self.ui_state.interaction.pressed_id = None
            self.ui_state.interaction.active_slider_id = None
            self.sm.on_hover_change(False, self.config)
            self.paint()
            self._tick_animation()
            return 0

        if msg == WM_LBUTTONDOWN:
            px, py = _signed_xy(lparam)
            lx, ly = px / self._dpi_scale, py / self._dpi_scale
            if self.ui_state.interaction.handle_pointer_down(lx, ly):
                self.paint()
            return 0

        if msg == WM_LBUTTONUP:
            px, py = _signed_xy(lparam)
            lx, ly = px / self._dpi_scale, py / self._dpi_scale
            clicked_input = self.ui_state.interaction.handle_click(lx, ly)
            if clicked_input is not None:
                self.set_smart_focus(True, clicked_input)
            elif self._smart_focus_active:
                self.set_smart_focus(False)
            else:
                self.paint()
            return 0

        if msg == WM_KEYDOWN and self._smart_focus_active:
            inp = self.ui_state.active_input()
            if wparam == VK_ESCAPE:
                self.set_smart_focus(False)
                return 0
            if wparam == VK_RETURN:
                if self.ui_state.focused_input_id == "google_search":
                    self._on_submit_search()
                elif self.ui_state.focused_input_id == "task_input":
                    self._on_add_task()
                elif self.ui_state.focused_input_id == "app_url_input":
                    self._on_pin_text_shortcut()
                else:
                    self.set_smart_focus(False)
                return 0
            if inp is not None:
                if wparam == VK_BACK:
                    inp.backspace()
                elif wparam == VK_DELETE:
                    inp.delete()
                elif wparam == VK_LEFT:
                    inp.cursor = max(0, inp.cursor - 1)
                elif wparam == VK_RIGHT:
                    inp.cursor = min(len(inp.text), inp.cursor + 1)
                self.paint()
            return 0

        if msg == WM_CHAR and self._smart_focus_active:
            inp = self.ui_state.active_input()
            ch = chr(wparam)
            if inp is not None and ord(ch) >= 32 and ch != "\x7f":
                inp.insert_text(ch)
                self.paint()
            return 0

        if msg == WM_TIMER:
            if wparam == TIMER_ANIM_ID:
                self._tick_animation()
            elif wparam == TIMER_SLOW_ID:
                due = self.tasks.pop_due_reminders(time.time())
                if due:
                    self.sm.trigger_peek("Task Reminder", due[0].title, self.config)
                    self._tick_animation()
                if self.timer.tick():
                    self.sm.trigger_peek("Timer Done", self.timer.label, self.config)
                    self._tick_animation()
                elif self.sm.state in (NotchState.PEEK, NotchState.EXPANDED) or self.timer.running:
                    self._sync_data_to_ui(refresh_heavy=(self.sm.state == NotchState.EXPANDED))
                    self.paint()
            return 0

        if msg == WM_HOTKEY:
            if wparam == HOTKEY_TOGGLE_HIDE:
                self._on_hide_manual()
            elif wparam == HOTKEY_SEARCH:
                if self.sm.state == NotchState.HIDDEN:
                    self.sm.manual_hidden = False
                self.sm.expand(ExpandedTab.OVERVIEW, self.config)
                self._sync_data_to_ui(refresh_heavy=True)
                self._tick_animation()
                self.set_smart_focus(True, "google_search")
            return 0

        if msg == WM_TRAY and lparam == WM_RBUTTONUP:
            self._show_tray_menu()
            return 0

        if msg == WM_COMMAND:
            cmd = wparam & 0xFFFF
            if cmd == IDM_TOGGLE:
                self._on_toggle_expand()
            elif cmd == IDM_MODE_FIXED:
                self._set_visibility_mode(VisibilityMode.FIXED)
            elif cmd == IDM_MODE_SMART:
                self._set_visibility_mode(VisibilityMode.SMART)
            elif cmd == IDM_MODE_PEEK:
                self._set_visibility_mode(VisibilityMode.PEEK)
            elif cmd == IDM_PRIVACY:
                self.toggle_capture_privacy()
            elif cmd == IDM_EXIT:
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
            return 0

        if msg == WM_DPICHANGED:
            dpi = wparam & 0xFFFF
            if dpi:
                self._dpi_scale = max(1.0, dpi / 96.0)
            self._update_monitor_bounds()
            self.paint()
            return 0

        if msg == WM_APP_REFRESH:
            self.paint()
            return 0

        if msg == WM_CLOSE:
            user32.DestroyWindow(hwnd)
            return 0

        if msg == WM_DESTROY:
            self._cleanup_resources()
            user32.PostQuitMessage(0)
            return 0

        return user32.DefWindowProcW(hwnd, msg, wparam, lparam)

    def _cleanup_resources(self) -> None:
        self._set_anim_timer(False)
        if self.hwnd:
            user32.KillTimer(self.hwnd, TIMER_SLOW_ID)
            if self._enable_hotkeys:
                user32.UnregisterHotKey(self.hwnd, HOTKEY_TOGGLE_HIDE)
                user32.UnregisterHotKey(self.hwnd, HOTKEY_SEARCH)
            if self._drop_registered:
                ole32.RevokeDragDrop(self.hwnd)
                self._drop_registered = False
            self._remove_tray_icon()
            _HWND_INSTANCES.pop(int(self.hwnd), None)
        if self._win_event_hook:
            user32.UnhookWinEvent(self._win_event_hook)
            self._win_event_hook = None
        self.discord.stop()
        self.canvas.dispose()
        try:
            self.conn.close()
        except Exception:
            pass
        self.hwnd = None

    def destroy(self) -> None:
        if self.hwnd:
            user32.DestroyWindow(self.hwnd)
        else:
            self._cleanup_resources()

    def run(self) -> None:
        msg = wintypes.MSG()
        while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
            user32.TranslateMessage(ctypes.byref(msg))
            user32.DispatchMessageW(ctypes.byref(msg))
