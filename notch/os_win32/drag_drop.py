from __future__ import annotations

import ctypes
from ctypes import wintypes
from typing import Callable

from notch.os_win32.com_helpers import GUID

ole32 = ctypes.WinDLL("ole32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
shell32 = ctypes.WinDLL("shell32", use_last_error=True)

CF_HDROP = 15
DVASPECT_CONTENT = 1
TYMED_HGLOBAL = 1
GMEM_MOVEABLE = 0x0002
GMEM_ZEROINIT = 0x0040
DROPEFFECT_NONE = 0
DROPEFFECT_COPY = 1
S_OK = 0
S_FALSE = 1
E_NOTIMPL = -2147467263
DV_E_FORMATETC = -2147221404


class DROPFILES(ctypes.Structure):
    _fields_ = [("pFiles", wintypes.DWORD), ("pt", wintypes.POINT), ("fNC", wintypes.BOOL), ("fWide", wintypes.BOOL)]


class FORMATETC(ctypes.Structure):
    _fields_ = [("cfFormat", wintypes.WORD), ("ptd", ctypes.c_void_p), ("dwAspect", wintypes.DWORD), ("lindex", wintypes.LONG), ("tymed", wintypes.DWORD)]


class STGMEDIUM(ctypes.Structure):
    _fields_ = [("tymed", wintypes.DWORD), ("hGlobal", ctypes.c_void_p), ("pUnkForRelease", ctypes.c_void_p)]


ole32.OleInitialize.argtypes = [ctypes.c_void_p]
ole32.OleInitialize.restype = ctypes.c_long
ole32.RegisterDragDrop.argtypes = [wintypes.HWND, ctypes.c_void_p]
ole32.RegisterDragDrop.restype = ctypes.c_long
ole32.RevokeDragDrop.argtypes = [wintypes.HWND]
ole32.RevokeDragDrop.restype = ctypes.c_long
ole32.DoDragDrop.argtypes = [ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
ole32.DoDragDrop.restype = ctypes.c_long
ole32.ReleaseStgMedium.argtypes = [ctypes.POINTER(STGMEDIUM)]
kernel32.GlobalAlloc.argtypes = [wintypes.UINT, ctypes.c_size_t]
kernel32.GlobalAlloc.restype = ctypes.c_void_p
kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalUnlock.argtypes = [ctypes.c_void_p]
kernel32.GlobalFree.argtypes = [ctypes.c_void_p]
shell32.DragQueryFileW.argtypes = [ctypes.c_void_p, wintypes.UINT, wintypes.LPWSTR, wintypes.UINT]
shell32.DragQueryFileW.restype = wintypes.UINT

IID_IUNKNOWN = GUID.from_str("{00000000-0000-0000-C000-000000000046}")
IID_IDROPTARGET = GUID.from_str("{00000122-0000-0000-C000-000000000046}")
IID_IDROPSOURCE = GUID.from_str("{00000121-0000-0000-C000-000000000046}")
IID_IDATAOBJECT = GUID.from_str("{0000010E-0000-0000-C000-000000000046}")


def ensure_ole_initialized() -> None:
    ole32.OleInitialize(None)


def build_cf_hdrop_hglobal(paths: list[str]) -> int:
    payload = ("\0".join(paths) + "\0\0").encode("utf-16-le")
    header_sz = ctypes.sizeof(DROPFILES)
    hmem = kernel32.GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, header_sz + len(payload))
    ptr = kernel32.GlobalLock(hmem)
    df = DROPFILES(pFiles=header_sz, pt=wintypes.POINT(0, 0), fNC=False, fWide=True)
    ctypes.memmove(ptr, ctypes.byref(df), header_sz)
    ctypes.memmove(ptr + header_sz, payload, len(payload))
    kernel32.GlobalUnlock(hmem)
    return int(hmem)


def extract_paths_from_hdrop(hdrop: int) -> list[str]:
    count = shell32.DragQueryFileW(hdrop, 0xFFFFFFFF, None, 0)
    out: list[str] = []
    for idx in range(count):
        length = shell32.DragQueryFileW(hdrop, idx, None, 0)
        buf = ctypes.create_unicode_buffer(length + 2)
        shell32.DragQueryFileW(hdrop, idx, buf, length + 2)
        if buf.value:
            out.append(buf.value)
    return out


def _guid_eq(riid_ptr: ctypes.c_void_p, target: GUID) -> bool:
    if not riid_ptr:
        return False
    return ctypes.string_at(riid_ptr, 16) == bytes(target)


class OleDropTarget:
    def __init__(self, on_drag_enter: Callable[[], None], on_drop_paths: Callable[[list[str]], None]) -> None:
        self._on_drag_enter = on_drag_enter
        self._on_drop_paths = on_drop_paths
        self._ref_count = 1
        self._callbacks = [
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(self._qi),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(self._add_ref),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(self._release),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, wintypes.POINT, ctypes.POINTER(wintypes.DWORD))(self._drag_enter),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.DWORD, wintypes.POINT, ctypes.POINTER(wintypes.DWORD))(self._drag_over),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p)(self._drag_leave),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, wintypes.POINT, ctypes.POINTER(wintypes.DWORD))(self._drop),
        ]
        self._vtable = (ctypes.c_void_p * len(self._callbacks))(*[ctypes.cast(cb, ctypes.c_void_p) for cb in self._callbacks])
        self._vtable_ptr = ctypes.pointer(self._vtable)
        self.com_ptr = ctypes.cast(ctypes.byref(self._vtable_ptr), ctypes.c_void_p)

    def _qi(self, _this, riid, ppv) -> int:
        if _guid_eq(riid, IID_IUNKNOWN) or _guid_eq(riid, IID_IDROPTARGET):
            ppv[0] = self.com_ptr.value
            self._ref_count += 1
            return S_OK
        ppv[0] = None
        return -2147467262

    def _add_ref(self, _this) -> int:
        self._ref_count += 1
        return self._ref_count

    def _release(self, _this) -> int:
        self._ref_count = max(0, self._ref_count - 1)
        return self._ref_count

    def _drag_enter(self, _this, _data_obj, _ks, _pt, effect) -> int:
        effect[0] = DROPEFFECT_COPY
        self._on_drag_enter()
        return S_OK

    def _drag_over(self, _this, _ks, _pt, effect) -> int:
        effect[0] = DROPEFFECT_COPY
        return S_OK

    def _drag_leave(self, _this) -> int:
        return S_OK

    def _drop(self, _this, data_obj, _ks, _pt, effect) -> int:
        effect[0] = DROPEFFECT_NONE
        if not data_obj:
            return S_OK
        vtbl = ctypes.cast(data_obj, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p))).contents
        get_data = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), ctypes.POINTER(STGMEDIUM))(vtbl[3])
        fmt = FORMATETC(cfFormat=CF_HDROP, ptd=None, dwAspect=DVASPECT_CONTENT, lindex=-1, tymed=TYMED_HGLOBAL)
        med = STGMEDIUM()
        if get_data(data_obj, ctypes.byref(fmt), ctypes.byref(med)) == S_OK and med.hGlobal:
            paths = extract_paths_from_hdrop(med.hGlobal)
            ole32.ReleaseStgMedium(ctypes.byref(med))
            if paths:
                effect[0] = DROPEFFECT_COPY
                self._on_drop_paths(paths)
        return S_OK


class _DropSource:
    def __init__(self) -> None:
        self._ref = 1
        self._cbs = [
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(self._qi),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(lambda _t: self._add()),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(lambda _t: self._rel()),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.BOOL, wintypes.DWORD)(self._qcd),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.DWORD)(lambda _t, _e: 0x00040202),
        ]
        self._vt = (ctypes.c_void_p * len(self._cbs))(*[ctypes.cast(c, ctypes.c_void_p) for c in self._cbs])
        self._vp = ctypes.pointer(self._vt)
        self.com_ptr = ctypes.cast(ctypes.byref(self._vp), ctypes.c_void_p)

    def _add(self) -> int:
        self._ref += 1
        return self._ref

    def _rel(self) -> int:
        self._ref = max(0, self._ref - 1)
        return self._ref

    def _qi(self, _t, riid, ppv) -> int:
        if _guid_eq(riid, IID_IUNKNOWN) or _guid_eq(riid, IID_IDROPSOURCE):
            ppv[0] = self.com_ptr.value
            self._ref += 1
            return S_OK
        ppv[0] = None
        return -2147467262

    def _qcd(self, _t, esc, ks) -> int:
        if esc:
            return 0x00040201  # DRAGDROP_S_CANCEL
        if not (ks & 0x1):
            return 0x00040200  # DRAGDROP_S_DROP
        return S_OK


class _FileDataObject:
    def __init__(self, paths: list[str]) -> None:
        self._paths = paths
        self._ref = 1
        self._cbs = [
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(self._qi),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(lambda _t: self._add()),
            ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(lambda _t: self._rel()),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), ctypes.POINTER(STGMEDIUM))(self._get_data),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), ctypes.POINTER(STGMEDIUM))(lambda _t, _f, _m: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC))(self._query_get_data),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), ctypes.POINTER(FORMATETC))(lambda _t, _f, _o: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), ctypes.POINTER(STGMEDIUM), wintypes.BOOL)(lambda _t, _f, _m, _r: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(ctypes.c_void_p))(lambda _t, _d, _e: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(FORMATETC), wintypes.DWORD, ctypes.c_void_p, ctypes.POINTER(wintypes.DWORD))(lambda _t, _f, _a, _s, _c: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.DWORD)(lambda _t, _c: E_NOTIMPL),
            ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p))(lambda _t, _e: E_NOTIMPL),
        ]
        self._vt = (ctypes.c_void_p * len(self._cbs))(*[ctypes.cast(c, ctypes.c_void_p) for c in self._cbs])
        self._vp = ctypes.pointer(self._vt)
        self.com_ptr = ctypes.cast(ctypes.byref(self._vp), ctypes.c_void_p)

    def _add(self) -> int:
        self._ref += 1
        return self._ref

    def _rel(self) -> int:
        self._ref = max(0, self._ref - 1)
        return self._ref

    def _qi(self, _t, riid, ppv) -> int:
        if _guid_eq(riid, IID_IUNKNOWN) or _guid_eq(riid, IID_IDATAOBJECT):
            ppv[0] = self.com_ptr.value
            self._ref += 1
            return S_OK
        ppv[0] = None
        return -2147467262

    def _query_get_data(self, _t, fmt_ptr) -> int:
        fmt = fmt_ptr.contents
        return S_OK if (fmt.cfFormat == CF_HDROP and (fmt.tymed & TYMED_HGLOBAL)) else DV_E_FORMATETC

    def _get_data(self, _t, fmt_ptr, med_ptr) -> int:
        if self._query_get_data(_t, fmt_ptr) != S_OK:
            return DV_E_FORMATETC
        med = med_ptr.contents
        med.tymed = TYMED_HGLOBAL
        med.hGlobal = build_cf_hdrop_hglobal(self._paths)
        med.pUnkForRelease = None
        return S_OK


def start_cf_hdrop_drag(paths: list[str]) -> int:
    if not paths:
        return DROPEFFECT_NONE
    ensure_ole_initialized()
    data_obj = _FileDataObject(paths)
    drop_src = _DropSource()
    effect = wintypes.DWORD(0)
    ole32.DoDragDrop(data_obj.com_ptr, drop_src.com_ptr, DROPEFFECT_COPY, ctypes.byref(effect))
    return int(effect.value)
