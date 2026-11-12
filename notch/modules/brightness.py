from __future__ import annotations

import ctypes
from ctypes import wintypes
from dataclasses import dataclass

dxva2 = ctypes.WinDLL("dxva2", use_last_error=True)
user32 = ctypes.WinDLL("user32", use_last_error=True)
MONITOR_DEFAULTTOPRIMARY = 1


class PHYSICAL_MONITOR(ctypes.Structure):
    _fields_ = [("hPhysicalMonitor", wintypes.HANDLE), ("szPhysicalMonitorDescription", wintypes.WCHAR * 128)]


dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR.argtypes = [wintypes.HMONITOR, ctypes.POINTER(wintypes.DWORD)]
dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR.restype = wintypes.BOOL
dxva2.GetPhysicalMonitorsFromHMONITOR.argtypes = [wintypes.HMONITOR, wintypes.DWORD, ctypes.POINTER(PHYSICAL_MONITOR)]
dxva2.GetPhysicalMonitorsFromHMONITOR.restype = wintypes.BOOL
dxva2.DestroyPhysicalMonitors.argtypes = [wintypes.DWORD, ctypes.POINTER(PHYSICAL_MONITOR)]
dxva2.GetMonitorBrightness.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD)]
dxva2.GetMonitorBrightness.restype = wintypes.BOOL
dxva2.SetMonitorBrightness.argtypes = [wintypes.HANDLE, wintypes.DWORD]
dxva2.SetMonitorBrightness.restype = wintypes.BOOL
user32.MonitorFromWindow.argtypes = [wintypes.HWND, wintypes.DWORD]
user32.MonitorFromWindow.restype = wintypes.HMONITOR


@dataclass
class BrightnessState:
    level: float = 0.5
    backend: str = "ddc"
    supported: bool = False


class BrightnessController:
    def get_brightness(self, hwnd: wintypes.HWND | None = None) -> BrightnessState:
        mon = user32.MonitorFromWindow(hwnd or 0, MONITOR_DEFAULTTOPRIMARY)
        cnt = wintypes.DWORD(0)
        if not dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR(mon, ctypes.byref(cnt)) or cnt.value == 0:
            return BrightnessState(level=0.5, backend="unsupported", supported=False)
        arr = (PHYSICAL_MONITOR * cnt.value)()
        if not dxva2.GetPhysicalMonitorsFromHMONITOR(mon, cnt.value, arr):
            return BrightnessState(level=0.5, backend="unsupported", supported=False)
        try:
            lo, cur, hi = wintypes.DWORD(0), wintypes.DWORD(0), wintypes.DWORD(100)
            if dxva2.GetMonitorBrightness(arr[0].hPhysicalMonitor, ctypes.byref(lo), ctypes.byref(cur), ctypes.byref(hi)):
                span = max(1, hi.value - lo.value)
                return BrightnessState(level=max(0.0, min(1.0, (cur.value - lo.value) / span)), backend="DDC/CI", supported=True)
            return BrightnessState(level=0.5, backend="unsupported", supported=False)
        finally:
            dxva2.DestroyPhysicalMonitors(cnt.value, arr)

    def set_brightness(self, ratio: float, hwnd: wintypes.HWND | None = None) -> bool:
        clamped = max(0.0, min(1.0, float(ratio)))
        mon = user32.MonitorFromWindow(hwnd or 0, MONITOR_DEFAULTTOPRIMARY)
        cnt = wintypes.DWORD(0)
        if not dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR(mon, ctypes.byref(cnt)) or cnt.value == 0:
            return False
        arr = (PHYSICAL_MONITOR * cnt.value)()
        if not dxva2.GetPhysicalMonitorsFromHMONITOR(mon, cnt.value, arr):
            return False
        try:
            lo, cur, hi = wintypes.DWORD(0), wintypes.DWORD(0), wintypes.DWORD(100)
            if dxva2.GetMonitorBrightness(arr[0].hPhysicalMonitor, ctypes.byref(lo), ctypes.byref(cur), ctypes.byref(hi)):
                target = int(round(lo.value + clamped * max(1, hi.value - lo.value)))
                return bool(dxva2.SetMonitorBrightness(arr[0].hPhysicalMonitor, target))
            return False
        finally:
            dxva2.DestroyPhysicalMonitors(cnt.value, arr)
