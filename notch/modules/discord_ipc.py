from __future__ import annotations

import ctypes
import json
import os
import struct
import threading
import uuid
from ctypes import wintypes
from dataclasses import dataclass
from typing import Callable

from notch.core.config import AppConfig
from notch.modules.audio import AudioController

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
user32 = ctypes.WinDLL("user32", use_last_error=True)

TH32CS_SNAPPROCESS = 0x00000002
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value
GENERIC_READ = 0x80000000
GENERIC_WRITE = 0x40000000
OPEN_EXISTING = 3
FILE_FLAG_OVERLAPPED = 0x40000000
ERROR_IO_PENDING = 997
WAIT_OBJECT_0 = 0
KEYEVENTF_KEYUP = 0x0002


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD), ("th32ProcessID", wintypes.DWORD),
        ("th32DefaultHeapID", ctypes.c_size_t), ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
        ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG), ("dwFlags", wintypes.DWORD),
        ("szExeFile", wintypes.WCHAR * 260),
    ]


class OVERLAPPED(ctypes.Structure):
    _fields_ = [("Internal", ctypes.c_size_t), ("InternalHigh", ctypes.c_size_t), ("Offset", wintypes.DWORD), ("OffsetHigh", wintypes.DWORD), ("hEvent", wintypes.HANDLE)]


kernel32.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
kernel32.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
kernel32.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
kernel32.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
kernel32.CreateFileW.restype = wintypes.HANDLE
kernel32.ReadFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(OVERLAPPED)]
kernel32.WriteFile.argtypes = [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(OVERLAPPED)]
kernel32.CreateEventW.argtypes = [ctypes.c_void_p, wintypes.BOOL, wintypes.BOOL, wintypes.LPCWSTR]
kernel32.CreateEventW.restype = wintypes.HANDLE
kernel32.SetEvent.argtypes = [wintypes.HANDLE]
kernel32.CancelIoEx.argtypes = [wintypes.HANDLE, ctypes.c_void_p]
kernel32.GetOverlappedResult.argtypes = [wintypes.HANDLE, ctypes.POINTER(OVERLAPPED), ctypes.POINTER(wintypes.DWORD), wintypes.BOOL]
kernel32.WaitForMultipleObjects.argtypes = [wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE), wintypes.BOOL, wintypes.DWORD]
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
user32.keybd_event.argtypes = [ctypes.c_ubyte, ctypes.c_ubyte, wintypes.DWORD, ctypes.c_size_t]


@dataclass
class DiscordVoiceState:
    discord_running: bool = False
    rpc_connected: bool = False
    in_call: bool = False
    muted: bool = False
    deafened: bool = False
    channel_name: str = ""
    mode_label: str = "Idle"


def is_discord_running() -> bool:
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if snap == INVALID_HANDLE_VALUE:
        return False
    try:
        pe = PROCESSENTRY32W(dwSize=ctypes.sizeof(PROCESSENTRY32W))
        if not kernel32.Process32FirstW(snap, ctypes.byref(pe)):
            return False
        while True:
            if pe.szExeFile.lower() == "discord.exe":
                return True
            if not kernel32.Process32NextW(snap, ctypes.byref(pe)):
                break
        return False
    finally:
        kernel32.CloseHandle(snap)


def send_hotkey_combo(combo: str) -> bool:
    parts = [p.strip().upper() for p in combo.split("+") if p.strip()]
    if not parts:
        return False
    vk_map = {"CTRL": 0x11, "CONTROL": 0x11, "ALT": 0x12, "SHIFT": 0x10}
    vks: list[int] = []
    for p in parts:
        if p in vk_map:
            vks.append(vk_map[p])
        elif len(p) == 1:
            vks.append(ord(p))
    for vk in vks:
        user32.keybd_event(vk, 0, 0, 0)
    for vk in reversed(vks):
        user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)
    return bool(vks)


class DiscordController:
    def __init__(self, config: AppConfig, audio: AudioController, on_state: Callable[[DiscordVoiceState], None]) -> None:
        self._config = config
        self._audio = audio
        self._on_state = on_state
        self._state = DiscordVoiceState()
        self._stop = threading.Event()
        self._stop_event = kernel32.CreateEventW(None, True, False, None)
        self._pipe: int | None = None
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        if self._thread is None:
            self._thread = threading.Thread(target=self._run, daemon=True)
            self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._stop_event:
            kernel32.SetEvent(self._stop_event)
        if self._pipe:
            kernel32.CancelIoEx(self._pipe, None)
        if self._thread:
            self._thread.join(timeout=1.5)
        if self._stop_event:
            kernel32.CloseHandle(self._stop_event)
            self._stop_event = None

    def toggle_mute(self) -> None:
        if self._state.rpc_connected and self._pipe:
            self._state.muted = not self._state.muted
            self._send_rpc("SET_VOICE_SETTINGS", {"mute": self._state.muted})
            self._emit()
            return
        if self._config.discord_mute_hotkey and send_hotkey_combo(self._config.discord_mute_hotkey):
            self._state.muted = not self._state.muted
        else:
            res = self._audio.toggle_exe_mute("Discord.exe")
            if res is not None:
                self._state.muted = res
        self._emit()

    def toggle_deafen(self) -> None:
        if self._state.rpc_connected and self._pipe:
            self._state.deafened = not self._state.deafened
            self._send_rpc("SET_VOICE_SETTINGS", {"deaf": self._state.deafened})
            self._emit()
            return
        if self._config.discord_deafen_hotkey and send_hotkey_combo(self._config.discord_deafen_hotkey):
            self._state.deafened = not self._state.deafened
            self._emit()

    def disconnect_call(self) -> None:
        if self._state.rpc_connected and self._pipe:
            self._send_rpc("SELECT_VOICE_CHANNEL", {"channel_id": None})

    def _emit(self) -> None:
        self._on_state(DiscordVoiceState(**self._state.__dict__))

    def _send_rpc(self, cmd: str, args: dict) -> None:
        if not self._pipe:
            return
        payload = json.dumps({"cmd": cmd, "args": args, "nonce": str(uuid.uuid4())}).encode("utf-8")
        frame = struct.pack("<II", 1, len(payload)) + payload
        self._write_overlapped(self._pipe, frame)

    def _write_overlapped(self, handle: int, data: bytes) -> bool:
        ev = kernel32.CreateEventW(None, True, False, None)
        if not ev:
            return False
        try:
            ov = OVERLAPPED(hEvent=ev)
            buf = (ctypes.c_char * len(data)).from_buffer_copy(data)
            written = wintypes.DWORD(0)
            ok = kernel32.WriteFile(handle, buf, len(data), ctypes.byref(written), ctypes.byref(ov))
            if not ok and ctypes.get_last_error() == ERROR_IO_PENDING:
                handles = (wintypes.HANDLE * 2)(ev, self._stop_event)
                if kernel32.WaitForMultipleObjects(2, handles, False, 1000) != WAIT_OBJECT_0:
                    kernel32.CancelIoEx(handle, ctypes.byref(ov))
                    return False
                kernel32.GetOverlappedResult(handle, ctypes.byref(ov), ctypes.byref(written), False)
            return True
        finally:
            kernel32.CloseHandle(ev)

    def _read_overlapped(self, handle: int, nbytes: int) -> bytes | None:
        ev = kernel32.CreateEventW(None, True, False, None)
        if not ev:
            return None
        try:
            out = bytearray()
            while len(out) < nbytes and not self._stop.is_set():
                rem = nbytes - len(out)
                buf = (ctypes.c_char * rem)()
                ov = OVERLAPPED(hEvent=ev)
                got = wintypes.DWORD(0)
                ok = kernel32.ReadFile(handle, buf, rem, ctypes.byref(got), ctypes.byref(ov))
                if not ok:
                    if ctypes.get_last_error() != ERROR_IO_PENDING:
                        return None
                    handles = (wintypes.HANDLE * 2)(ev, self._stop_event)
                    if kernel32.WaitForMultipleObjects(2, handles, False, 1500) != WAIT_OBJECT_0:
                        kernel32.CancelIoEx(handle, ctypes.byref(ov))
                        return None
                    if not kernel32.GetOverlappedResult(handle, ctypes.byref(ov), ctypes.byref(got), False):
                        return None
                if got.value == 0:
                    return None
                out.extend(bytes(buf[: got.value]))
            return bytes(out)
        finally:
            kernel32.CloseHandle(ev)

    def _run(self) -> None:
        client_id = os.environ.get("NOTCH_DISCORD_CLIENT_ID", "")
        while not self._stop.is_set():
            running = is_discord_running()
            self._state.discord_running = running
            if not running:
                self._state.rpc_connected = False
                self._state.in_call = False
                self._state.mode_label = "Idle"
                self._emit()
                self._stop.wait(4.0)
                continue

            if client_id and not self._pipe:
                for idx in range(4):
                    h = kernel32.CreateFileW(f"\\\\.\\pipe\\discord-ipc-{idx}", GENERIC_READ | GENERIC_WRITE, 0, None, OPEN_EXISTING, FILE_FLAG_OVERLAPPED, None)
                    if h != INVALID_HANDLE_VALUE:
                        self._pipe = h
                        hs = json.dumps({"v": 1, "client_id": client_id}).encode("utf-8")
                        self._write_overlapped(h, struct.pack("<II", 0, len(hs)) + hs)
                        self._state.rpc_connected = True
                        self._state.mode_label = "RPC"
                        break

            if self._pipe:
                hdr = self._read_overlapped(self._pipe, 8)
                if not hdr:
                    kernel32.CloseHandle(self._pipe)
                    self._pipe = None
                    self._state.rpc_connected = False
                else:
                    _op, ln = struct.unpack("<II", hdr)
                    body = self._read_overlapped(self._pipe, min(ln, 65536))
                    if body:
                        try:
                            msg = json.loads(body.decode("utf-8"))
                            data = msg.get("data") or {}
                            if "mute" in data:
                                self._state.muted = bool(data["mute"])
                            if "deaf" in data:
                                self._state.deafened = bool(data["deaf"])
                            if "channel_id" in data:
                                self._state.in_call = bool(data["channel_id"])
                        except Exception:
                            pass
                        self._emit()
                        continue

            self._state.mode_label = "Hotkey" if self._config.discord_mute_hotkey else "Audio"
            self._emit()
            self._stop.wait(3.0)
