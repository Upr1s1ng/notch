from __future__ import annotations

import ctypes
from ctypes import wintypes
from dataclasses import dataclass

from notch.os_win32.com_helpers import GUID, com_release, vtable_fn

ole32 = ctypes.WinDLL("ole32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

CLSID_MMDEVICE_ENUMERATOR = GUID.from_str("{BCDE0395-E52F-467C-8E3D-C4579291692E}")
IID_IMMDEVICE_ENUMERATOR = GUID.from_str("{A95664D2-9614-4F35-A746-DE8DB63617E6}")
IID_IAUDIO_ENDPOINT_VOLUME = GUID.from_str("{5CDF2C82-841E-4546-9722-0CF74078229A}")
IID_IAUDIO_SESSION_MANAGER2 = GUID.from_str("{77AA99A0-1BD6-484F-8BC7-2C654C9A9B6F}")
IID_IAUDIO_SESSION_CONTROL2 = GUID.from_str("{BFB7FF88-7239-4FC9-8FA2-07C950BE9C6D}")
IID_ISIMPLE_AUDIO_VOLUME = GUID.from_str("{87CE5498-68D6-44E5-9215-6DA47EF883D8}")

COINIT_APARTMENTTHREADED = 0x2
CLSCTX_ALL = 0x17
E_RENDER = 0
E_MULTIMEDIA = 1
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000

ole32.CoInitializeEx.argtypes = [ctypes.c_void_p, wintypes.DWORD]
ole32.CoCreateInstance.argtypes = [ctypes.POINTER(GUID), ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(GUID), ctypes.POINTER(ctypes.c_void_p)]
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
psapi.GetModuleBaseNameW.argtypes = [wintypes.HANDLE, wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD]


@dataclass
class AudioSessionInfo:
    pid: int
    name: str
    exe_name: str
    volume: float
    muted: bool


def _process_name(pid: int) -> str:
    if pid <= 0:
        return "System Sounds"
    h = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not h:
        return f"App ({pid})"
    try:
        buf = ctypes.create_unicode_buffer(260)
        if psapi.GetModuleBaseNameW(h, None, buf, 260):
            return buf.value
        return f"App ({pid})"
    finally:
        kernel32.CloseHandle(h)


class AudioController:
    def __init__(self) -> None:
        ole32.CoInitializeEx(None, COINIT_APARTMENTTHREADED)

    def _default_endpoint(self) -> ctypes.c_void_p | None:
        enum_ptr = ctypes.c_void_p()
        hr = ole32.CoCreateInstance(ctypes.byref(CLSID_MMDEVICE_ENUMERATOR), None, CLSCTX_ALL, ctypes.byref(IID_IMMDEVICE_ENUMERATOR), ctypes.byref(enum_ptr))
        if hr != 0 or not enum_ptr:
            return None
        dev_ptr = ctypes.c_void_p()
        try:
            hr = vtable_fn(enum_ptr, 4, ctypes.c_long, ctypes.c_int, ctypes.c_int, ctypes.POINTER(ctypes.c_void_p))(enum_ptr, E_RENDER, E_MULTIMEDIA, ctypes.byref(dev_ptr))
            return dev_ptr if hr == 0 else None
        finally:
            com_release(enum_ptr)

    def _endpoint_volume(self) -> ctypes.c_void_p | None:
        dev = self._default_endpoint()
        if not dev:
            return None
        vol = ctypes.c_void_p()
        try:
            hr = vtable_fn(dev, 3, ctypes.c_long, ctypes.POINTER(GUID), wintypes.DWORD, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(dev, ctypes.byref(IID_IAUDIO_ENDPOINT_VOLUME), CLSCTX_ALL, None, ctypes.byref(vol))
            return vol if hr == 0 else None
        finally:
            com_release(dev)

    def get_master_volume(self) -> tuple[float, bool]:
        vol_ptr = self._endpoint_volume()
        if not vol_ptr:
            return (0.5, False)
        try:
            lvl = ctypes.c_float(0.5)
            muted = wintypes.BOOL(False)
            vtable_fn(vol_ptr, 9, ctypes.c_long, ctypes.POINTER(ctypes.c_float))(vol_ptr, ctypes.byref(lvl))
            vtable_fn(vol_ptr, 15, ctypes.c_long, ctypes.POINTER(wintypes.BOOL))(vol_ptr, ctypes.byref(muted))
            return (max(0.0, min(1.0, float(lvl.value))), bool(muted.value))
        finally:
            com_release(vol_ptr)

    def set_master_volume(self, ratio: float) -> None:
        vol_ptr = self._endpoint_volume()
        if not vol_ptr:
            return
        try:
            clamped = max(0.0, min(1.0, float(ratio)))
            vtable_fn(vol_ptr, 7, ctypes.c_long, ctypes.c_float, ctypes.c_void_p)(vol_ptr, clamped, None)
        finally:
            com_release(vol_ptr)

    def toggle_master_mute(self) -> bool:
        vol_ptr = self._endpoint_volume()
        if not vol_ptr:
            return False
        try:
            muted = wintypes.BOOL(False)
            vtable_fn(vol_ptr, 15, ctypes.c_long, ctypes.POINTER(wintypes.BOOL))(vol_ptr, ctypes.byref(muted))
            nxt = not bool(muted.value)
            vtable_fn(vol_ptr, 14, ctypes.c_long, wintypes.BOOL, ctypes.c_void_p)(vol_ptr, nxt, None)
            return nxt
        finally:
            com_release(vol_ptr)

    def _iter_sessions(self):
        dev = self._default_endpoint()
        if not dev:
            return
        mgr = ctypes.c_void_p()
        enum = ctypes.c_void_p()
        try:
            if vtable_fn(dev, 3, ctypes.c_long, ctypes.POINTER(GUID), wintypes.DWORD, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(dev, ctypes.byref(IID_IAUDIO_SESSION_MANAGER2), CLSCTX_ALL, None, ctypes.byref(mgr)) != 0:
                return
            if vtable_fn(mgr, 5, ctypes.c_long, ctypes.POINTER(ctypes.c_void_p))(mgr, ctypes.byref(enum)) != 0:
                return
            cnt = ctypes.c_int(0)
            vtable_fn(enum, 3, ctypes.c_long, ctypes.POINTER(ctypes.c_int))(enum, ctypes.byref(cnt))
            for idx in range(cnt.value):
                ctl = ctypes.c_void_p()
                if vtable_fn(enum, 4, ctypes.c_long, ctypes.c_int, ctypes.POINTER(ctypes.c_void_p))(enum, idx, ctypes.byref(ctl)) != 0 or not ctl:
                    continue
                ctl2 = ctypes.c_void_p()
                simp = ctypes.c_void_p()
                try:
                    if vtable_fn(ctl, 0, ctypes.c_long, ctypes.POINTER(GUID), ctypes.POINTER(ctypes.c_void_p))(ctl, ctypes.byref(IID_IAUDIO_SESSION_CONTROL2), ctypes.byref(ctl2)) == 0 and ctl2:
                        pid = wintypes.DWORD(0)
                        vtable_fn(ctl2, 14, ctypes.c_long, ctypes.POINTER(wintypes.DWORD))(ctl2, ctypes.byref(pid))
                        if vtable_fn(ctl, 0, ctypes.c_long, ctypes.POINTER(GUID), ctypes.POINTER(ctypes.c_void_p))(ctl, ctypes.byref(IID_ISIMPLE_AUDIO_VOLUME), ctypes.byref(simp)) == 0 and simp:
                            yield pid.value, simp
                finally:
                    com_release(simp)
                    com_release(ctl2)
                    com_release(ctl)
        finally:
            com_release(enum)
            com_release(mgr)
            com_release(dev)

    def list_app_sessions(self) -> list[AudioSessionInfo]:
        seen: dict[int, AudioSessionInfo] = {}
        for pid, simp in self._iter_sessions():
            if pid <= 0:
                continue
            lvl = ctypes.c_float(1.0)
            muted = wintypes.BOOL(False)
            vtable_fn(simp, 4, ctypes.c_long, ctypes.POINTER(ctypes.c_float))(simp, ctypes.byref(lvl))
            vtable_fn(simp, 6, ctypes.c_long, ctypes.POINTER(wintypes.BOOL))(simp, ctypes.byref(muted))
            exe = _process_name(pid)
            label = exe.removesuffix(".exe").removesuffix(".EXE")
            seen[pid] = AudioSessionInfo(pid=pid, name=label, exe_name=exe, volume=max(0.0, min(1.0, float(lvl.value))), muted=bool(muted.value))
        return list(seen.values())

    def set_app_volume(self, pid: int, ratio: float) -> None:
        clamped = max(0.0, min(1.0, float(ratio)))
        for s_pid, simp in self._iter_sessions():
            if s_pid == pid:
                vtable_fn(simp, 3, ctypes.c_long, ctypes.c_float, ctypes.c_void_p)(simp, clamped, None)

    def toggle_app_mute(self, pid: int, mute: bool) -> None:
        for s_pid, simp in self._iter_sessions():
            if s_pid == pid:
                vtable_fn(simp, 5, ctypes.c_long, wintypes.BOOL, ctypes.c_void_p)(simp, mute, None)

    def toggle_exe_mute(self, exe_name: str) -> bool | None:
        target = exe_name.lower()
        toggled: bool | None = None
        for pid, simp in self._iter_sessions():
            if _process_name(pid).lower() == target:
                muted = wintypes.BOOL(False)
                vtable_fn(simp, 6, ctypes.c_long, ctypes.POINTER(wintypes.BOOL))(simp, ctypes.byref(muted))
                if toggled is None:
                    toggled = not bool(muted.value)
                vtable_fn(simp, 5, ctypes.c_long, wintypes.BOOL, ctypes.c_void_p)(simp, toggled, None)
        return toggled
