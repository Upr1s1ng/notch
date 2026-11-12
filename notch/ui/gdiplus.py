from __future__ import annotations

import ctypes
import hashlib
from ctypes import wintypes

from notch.os_win32.com_helpers import com_release
from notch.os_win32.icons import extract_target_hicon, user32 as icon_user32
from notch.os_win32.winapi import (
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    DIB_RGB_COLORS, ULW_ALPHA, gdi32, user32,
)

gdiplus = ctypes.WinDLL("gdiplus", use_last_error=True)
shlwapi = ctypes.WinDLL("shlwapi", use_last_error=True)

PIXEL_FORMAT_32BPP_PARGB = 0x000E200B
SMOOTHING_MODE_ANTI_ALIAS = 4
TEXT_RENDERING_ANTI_ALIAS_GRID_FIT = 3
UNIT_PIXEL = 2
STRING_TRIMMING_ELLIPSIS_CHAR = 3
STRING_FORMAT_NO_WRAP = 0x1000


class GdiplusStartupInput(ctypes.Structure):
    _fields_ = [("GdiplusVersion", wintypes.UINT), ("DebugEventCallback", ctypes.c_void_p), ("SuppressBackgroundThreads", wintypes.BOOL), ("SuppressExternalCodecs", wintypes.BOOL)]


class RectF(ctypes.Structure):
    _fields_ = [("X", ctypes.c_float), ("Y", ctypes.c_float), ("Width", ctypes.c_float), ("Height", ctypes.c_float)]


class PointF(ctypes.Structure):
    _fields_ = [("X", ctypes.c_float), ("Y", ctypes.c_float)]


def argb(a: int, r: int, g: int, b: int) -> int:
    return ((a & 0xFF) << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)


def _configure() -> None:
    vp, fl, ci = ctypes.c_void_p, ctypes.c_float, ctypes.c_int
    gdiplus.GdiplusStartup.argtypes = [ctypes.POINTER(ctypes.c_size_t), ctypes.POINTER(GdiplusStartupInput), vp]
    gdiplus.GdiplusShutdown.argtypes = [ctypes.c_size_t]
    gdiplus.GdipCreateBitmapFromScan0.argtypes = [ci, ci, ci, ci, vp, ctypes.POINTER(vp)]
    gdiplus.GdipGetImageGraphicsContext.argtypes = [vp, ctypes.POINTER(vp)]
    gdiplus.GdipDisposeImage.argtypes = [vp]
    gdiplus.GdipDeleteGraphics.argtypes = [vp]
    gdiplus.GdipSetSmoothingMode.argtypes = [vp, ci]
    gdiplus.GdipSetTextRenderingHint.argtypes = [vp, ci]
    gdiplus.GdipCreateSolidFill.argtypes = [wintypes.DWORD, ctypes.POINTER(vp)]
    gdiplus.GdipCreateLineBrush.argtypes = [ctypes.POINTER(PointF), ctypes.POINTER(PointF), wintypes.DWORD, wintypes.DWORD, ci, ctypes.POINTER(vp)]
    gdiplus.GdipDeleteBrush.argtypes = [vp]
    gdiplus.GdipCreatePen1.argtypes = [wintypes.DWORD, fl, ci, ctypes.POINTER(vp)]
    gdiplus.GdipSetPenStartCap.argtypes = [vp, ci]
    gdiplus.GdipSetPenEndCap.argtypes = [vp, ci]
    gdiplus.GdipSetPenLineJoin.argtypes = [vp, ci]
    gdiplus.GdipDeletePen.argtypes = [vp]
    gdiplus.GdipCreatePath.argtypes = [ci, ctypes.POINTER(vp)]
    gdiplus.GdipDeletePath.argtypes = [vp]
    gdiplus.GdipAddPathArc.argtypes = [vp, fl, fl, fl, fl, fl, fl]
    gdiplus.GdipClosePathFigure.argtypes = [vp]
    gdiplus.GdipFillPath.argtypes = [vp, vp, vp]
    gdiplus.GdipDrawPath.argtypes = [vp, vp, vp]
    gdiplus.GdipFillEllipse.argtypes = [vp, vp, fl, fl, fl, fl]
    gdiplus.GdipDrawEllipse.argtypes = [vp, vp, fl, fl, fl, fl]
    gdiplus.GdipFillPolygon.argtypes = [vp, vp, ctypes.POINTER(PointF), ci, ci]
    gdiplus.GdipDrawLines.argtypes = [vp, vp, ctypes.POINTER(PointF), ci]
    gdiplus.GdipDrawLine.argtypes = [vp, vp, fl, fl, fl, fl]
    gdiplus.GdipDrawArc.argtypes = [vp, vp, fl, fl, fl, fl, fl, fl]
    gdiplus.GdipCreateFontFamilyFromName.argtypes = [wintypes.LPCWSTR, vp, ctypes.POINTER(vp)]
    gdiplus.GdipDeleteFontFamily.argtypes = [vp]
    gdiplus.GdipCreateFont.argtypes = [vp, fl, ci, ci, ctypes.POINTER(vp)]
    gdiplus.GdipDeleteFont.argtypes = [vp]
    gdiplus.GdipCreateStringFormat.argtypes = [ci, wintypes.WORD, ctypes.POINTER(vp)]
    gdiplus.GdipDeleteStringFormat.argtypes = [vp]
    gdiplus.GdipSetStringFormatAlign.argtypes = [vp, ci]
    gdiplus.GdipSetStringFormatLineAlign.argtypes = [vp, ci]
    gdiplus.GdipSetStringFormatTrimming.argtypes = [vp, ci]
    gdiplus.GdipSetStringFormatFlags.argtypes = [vp, ci]
    gdiplus.GdipDrawString.argtypes = [vp, wintypes.LPCWSTR, ci, vp, ctypes.POINTER(RectF), vp, vp]
    gdiplus.GdipCreateBitmapFromStream.argtypes = [vp, ctypes.POINTER(vp)]
    gdiplus.GdipCreateBitmapFromHICON.argtypes = [wintypes.HICON, ctypes.POINTER(vp)]
    gdiplus.GdipDrawImageRect.argtypes = [vp, vp, fl, fl, fl, fl]
    gdiplus.GdipSetClipPath.argtypes = [vp, vp, ci]
    gdiplus.GdipResetClip.argtypes = [vp]
    gdiplus.GdipGetImageWidth.argtypes = [vp, ctypes.POINTER(wintypes.UINT)]
    gdiplus.GdipGetImageHeight.argtypes = [vp, ctypes.POINTER(wintypes.UINT)]
    gdiplus.GdipBitmapGetPixel.argtypes = [vp, ci, ci, ctypes.POINTER(wintypes.DWORD)]
    shlwapi.SHCreateMemStream.argtypes = [vp, wintypes.UINT]
    shlwapi.SHCreateMemStream.restype = vp


_configure()


def _build_round_rect_path(x: float, y: float, w: float, h: float, r: float) -> ctypes.c_void_p:
    path = ctypes.c_void_p(0)
    gdiplus.GdipCreatePath(0, ctypes.byref(path))
    radius = max(1.0, min(r, w * 0.5, h * 0.5))
    d = radius * 2.0
    gdiplus.GdipAddPathArc(path, x, y, d, d, 180.0, 90.0)
    gdiplus.GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0)
    gdiplus.GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0)
    gdiplus.GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0)
    gdiplus.GdipClosePathFigure(path)
    return path


def _sample_dominant(bmp: ctypes.c_void_p) -> int:
    w, h = wintypes.UINT(0), wintypes.UINT(0)
    gdiplus.GdipGetImageWidth(bmp, ctypes.byref(w))
    gdiplus.GdipGetImageHeight(bmp, ctypes.byref(h))
    if w.value == 0 or h.value == 0:
        return argb(255, 52, 211, 153)
    rs, gs, bs, cnt = 0, 0, 0, 0
    px = wintypes.DWORD(0)
    for gy in range(4):
        for gx in range(4):
            if gdiplus.GdipBitmapGetPixel(bmp, int((gx + 0.5) * w.value / 4), int((gy + 0.5) * h.value / 4), ctypes.byref(px)) == 0:
                v = px.value
                r, g, b = (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF
                if r + g + b > 60:
                    rs += r; gs += g; bs += b; cnt += 1
    if cnt == 0:
        return argb(255, 52, 211, 153)
    return argb(255, min(255, rs // cnt + 24), min(255, gs // cnt + 24), min(255, bs // cnt + 24))


class LayeredCanvas:
    def __init__(self) -> None:
        self._token = ctypes.c_size_t(0)
        si = GdiplusStartupInput(GdiplusVersion=1)
        gdiplus.GdiplusStartup(ctypes.byref(self._token), ctypes.byref(si), None)
        self._mem_dc = gdi32.CreateCompatibleDC(None)
        self._hbmp = None
        self._bits = ctypes.c_void_p(0)
        self._w = 0
        self._h = 0
        self._fonts: dict[tuple[str, float, bool], ctypes.c_void_p] = {}
        self._families: dict[str, ctypes.c_void_p] = {}
        self._imgs: dict[str, tuple[ctypes.c_void_p, int]] = {}
        self._icons: dict[str, ctypes.c_void_p] = {}

    def _ensure_surface(self, w: int, h: int) -> None:
        if w == self._w and h == self._h and self._hbmp:
            return
        if self._hbmp:
            gdi32.DeleteObject(self._hbmp)
            self._hbmp = None
        bmi = BITMAPINFO()
        bmi.bmiHeader = BITMAPINFOHEADER(
            biSize=ctypes.sizeof(BITMAPINFOHEADER), biWidth=w, biHeight=-h, biPlanes=1, biBitCount=32, biCompression=BI_RGB
        )
        bits = ctypes.c_void_p(0)
        self._hbmp = gdi32.CreateDIBSection(self._mem_dc, ctypes.byref(bmi), DIB_RGB_COLORS, ctypes.byref(bits), None, 0)
        gdi32.SelectObject(self._mem_dc, self._hbmp)
        self._bits = bits
        self._w, self._h = w, h

    def _get_font(self, family_name: str, size_px: float, bold: bool) -> ctypes.c_void_p:
        key = (family_name, round(size_px, 1), bold)
        if key in self._fonts:
            return self._fonts[key]
        fam = self._families.get(family_name)
        if not fam:
            fam = ctypes.c_void_p(0)
            gdiplus.GdipCreateFontFamilyFromName(family_name, None, ctypes.byref(fam))
            self._families[family_name] = fam
        font = ctypes.c_void_p(0)
        gdiplus.GdipCreateFont(fam, ctypes.c_float(size_px), 1 if bold else 0, UNIT_PIXEL, ctypes.byref(font))
        self._fonts[key] = font
        return font

    def _get_image(self, data: bytes | None) -> tuple[ctypes.c_void_p | None, int]:
        if not data:
            return (None, 0)
        key = hashlib.sha1(data).hexdigest()
        if key in self._imgs:
            return self._imgs[key]
        buf = (ctypes.c_ubyte * len(data)).from_buffer_copy(data)
        stream = shlwapi.SHCreateMemStream(buf, len(data))
        if not stream:
            return (None, 0)
        bmp = ctypes.c_void_p(0)
        try:
            gdiplus.GdipCreateBitmapFromStream(ctypes.c_void_p(stream), ctypes.byref(bmp))
        finally:
            com_release(ctypes.c_void_p(stream))
        if not bmp.value:
            return (None, 0)
        dom = _sample_dominant(bmp)
        if len(self._imgs) > 20:
            for old_bmp, _ in self._imgs.values():
                gdiplus.GdipDisposeImage(old_bmp)
            self._imgs.clear()
        self._imgs[key] = (bmp, dom)
        return (bmp, dom)

    def _get_target_icon(self, target: str) -> ctypes.c_void_p | None:
        if target in self._icons:
            return self._icons[target]
        hicon = extract_target_hicon(target)
        if not hicon:
            return None
        bmp = ctypes.c_void_p(0)
        try:
            gdiplus.GdipCreateBitmapFromHICON(hicon, ctypes.byref(bmp))
        finally:
            icon_user32.DestroyIcon(hicon)
        if not bmp.value:
            return None
        self._icons[target] = bmp
        return bmp

    def render_and_present(self, hwnd: wintypes.HWND, left: int, top: int, w: int, h: int, draw_fn) -> None:
        if w <= 0 or h <= 0:
            return
        self._ensure_surface(w, h)
        ctypes.memset(self._bits, 0, w * h * 4)
        bmp = ctypes.c_void_p(0)
        gfx = ctypes.c_void_p(0)
        gdiplus.GdipCreateBitmapFromScan0(w, h, w * 4, PIXEL_FORMAT_32BPP_PARGB, self._bits, ctypes.byref(bmp))
        gdiplus.GdipGetImageGraphicsContext(bmp, ctypes.byref(gfx))
        gdiplus.GdipSetSmoothingMode(gfx, SMOOTHING_MODE_ANTI_ALIAS)
        gdiplus.GdipSetTextRenderingHint(gfx, TEXT_RENDERING_ANTI_ALIAS_GRID_FIT)
        try:
            draw_fn(GdiPlusPainter(self, gfx))
        finally:
            gdiplus.GdipDeleteGraphics(gfx)
            gdiplus.GdipDisposeImage(bmp)
        dst = wintypes.POINT(left, top)
        sz = wintypes.SIZE(w, h)
        src = wintypes.POINT(0, 0)
        blend = BLENDFUNCTION(AC_SRC_OVER, 0, 255, AC_SRC_ALPHA)
        user32.UpdateLayeredWindow(hwnd, None, ctypes.byref(dst), ctypes.byref(sz), self._mem_dc, ctypes.byref(src), 0, ctypes.byref(blend), ULW_ALPHA)

    def dispose(self) -> None:
        for f in self._fonts.values():
            gdiplus.GdipDeleteFont(f)
        for fam in self._families.values():
            gdiplus.GdipDeleteFontFamily(fam)
        for bmp, _ in self._imgs.values():
            gdiplus.GdipDisposeImage(bmp)
        for bmp in self._icons.values():
            gdiplus.GdipDisposeImage(bmp)
        self._fonts.clear(); self._families.clear(); self._imgs.clear(); self._icons.clear()
        if self._hbmp:
            gdi32.DeleteObject(self._hbmp)
            self._hbmp = None
        if self._mem_dc:
            gdi32.DeleteDC(self._mem_dc)
            self._mem_dc = None
        if self._token.value:
            gdiplus.GdiplusShutdown(self._token)
            self._token = ctypes.c_size_t(0)


class GdiPlusPainter:
    def __init__(self, canvas: LayeredCanvas, gfx: ctypes.c_void_p) -> None:
        self._canvas = canvas
        self._gfx = gfx

    def fill_round_rect(self, x: float, y: float, w: float, h: float, r: float, color: int) -> None:
        path = _build_round_rect_path(x, y, w, h, r)
        brush = ctypes.c_void_p(0)
        gdiplus.GdipCreateSolidFill(color, ctypes.byref(brush))
        gdiplus.GdipFillPath(self._gfx, brush, path)
        gdiplus.GdipDeleteBrush(brush)
        gdiplus.GdipDeletePath(path)

    def fill_round_rect_gradient(self, x: float, y: float, w: float, h: float, r: float, top_col: int, bot_col: int) -> None:
        if w <= 0.0 or h <= 0.0:
            return
        p1, p2 = PointF(x, y), PointF(x, y + max(1.0, h))
        brush = ctypes.c_void_p(0)
        if gdiplus.GdipCreateLineBrush(ctypes.byref(p1), ctypes.byref(p2), top_col, bot_col, 0, ctypes.byref(brush)) != 0 or not brush:
            return
        path = _build_round_rect_path(x, y, w, h, r)
        gdiplus.GdipFillPath(self._gfx, brush, path)
        gdiplus.GdipDeletePath(path)
        gdiplus.GdipDeleteBrush(brush)

    def stroke_round_rect(self, x: float, y: float, w: float, h: float, r: float, color: int, thickness: float = 1.0) -> None:
        path = _build_round_rect_path(x, y, w, h, r)
        pen = ctypes.c_void_p(0)
        gdiplus.GdipCreatePen1(color, ctypes.c_float(thickness), UNIT_PIXEL, ctypes.byref(pen))
        gdiplus.GdipDrawPath(self._gfx, pen, path)
        gdiplus.GdipDeletePen(pen)
        gdiplus.GdipDeletePath(path)

    def fill_circle(self, cx: float, cy: float, radius: float, color: int) -> None:
        brush = ctypes.c_void_p(0)
        gdiplus.GdipCreateSolidFill(color, ctypes.byref(brush))
        gdiplus.GdipFillEllipse(self._gfx, brush, cx - radius, cy - radius, radius * 2.0, radius * 2.0)
        gdiplus.GdipDeleteBrush(brush)

    def stroke_circle(self, cx: float, cy: float, radius: float, color: int, width: float = 1.5) -> None:
        pen = ctypes.c_void_p(0)
        gdiplus.GdipCreatePen1(color, width, UNIT_PIXEL, ctypes.byref(pen))
        gdiplus.GdipDrawEllipse(self._gfx, pen, cx - radius, cy - radius, radius * 2.0, radius * 2.0)
        gdiplus.GdipDeletePen(pen)

    def fill_radial_glow(self, cx: float, cy: float, radius: float, argb_color: int, steps: int = 4) -> None:
        base_a = (argb_color >> 24) & 0xFF
        rgb = argb_color & 0x00FFFFFF
        if base_a <= 0 or radius <= 1.0:
            return
        for idx in range(steps, 0, -1):
            frac = idx / float(steps)
            sa = max(1, int(base_a * ((1.0 - frac * 0.72) / float(steps))))
            self.fill_circle(cx, cy, radius * frac, (sa << 24) | rgb)

    def draw_line(self, x1: float, y1: float, x2: float, y2: float, color: int, width: float = 1.0) -> None:
        pen = ctypes.c_void_p(0)
        gdiplus.GdipCreatePen1(color, ctypes.c_float(width), UNIT_PIXEL, ctypes.byref(pen))
        gdiplus.GdipDrawLine(self._gfx, pen, x1, y1, x2, y2)
        gdiplus.GdipDeletePen(pen)

    def draw_arc(self, x: float, y: float, w: float, h: float, start_deg: float, sweep_deg: float, color: int, width: float = 2.0) -> None:
        if abs(sweep_deg) < 0.5:
            return
        pen = ctypes.c_void_p(0)
        gdiplus.GdipCreatePen1(color, ctypes.c_float(width), UNIT_PIXEL, ctypes.byref(pen))
        gdiplus.GdipDrawArc(self._gfx, pen, x, y, w, h, start_deg, sweep_deg)
        gdiplus.GdipDeletePen(pen)

    def fill_polygon(self, points: list[tuple[float, float]], color: int) -> None:
        if len(points) < 3:
            return
        arr = (PointF * len(points))(*(PointF(float(px), float(py)) for px, py in points))
        brush = ctypes.c_void_p(0)
        gdiplus.GdipCreateSolidFill(color, ctypes.byref(brush))
        gdiplus.GdipFillPolygon(self._gfx, brush, arr, len(points), 0)
        gdiplus.GdipDeleteBrush(brush)

    def draw_polyline(self, points: list[tuple[float, float]], color: int, width: float = 1.6) -> None:
        if len(points) < 2:
            return
        arr = (PointF * len(points))(*(PointF(float(px), float(py)) for px, py in points))
        pen = ctypes.c_void_p(0)
        gdiplus.GdipCreatePen1(color, width, UNIT_PIXEL, ctypes.byref(pen))
        gdiplus.GdipSetPenStartCap(pen, 2)
        gdiplus.GdipSetPenEndCap(pen, 2)
        gdiplus.GdipSetPenLineJoin(pen, 2)
        gdiplus.GdipDrawLines(self._gfx, pen, arr, len(points))
        gdiplus.GdipDeletePen(pen)

    def get_image_dominant_color(self, data: bytes | None) -> int:
        return self._canvas._get_image(data)[1] if data else 0

    def draw_image_bytes(self, data: bytes | None, x: float, y: float, w: float, h: float, radius: float = 8.0) -> int:
        bmp, dom = self._canvas._get_image(data)
        if not bmp:
            return dom
        path = _build_round_rect_path(x, y, w, h, radius)
        gdiplus.GdipSetClipPath(self._gfx, path, 0)
        gdiplus.GdipDrawImageRect(self._gfx, bmp, x, y, w, h)
        gdiplus.GdipResetClip(self._gfx)
        gdiplus.GdipDeletePath(path)
        return dom

    def draw_target_icon(self, target: str, x: float, y: float, size: float) -> bool:
        bmp = self._canvas._get_target_icon(target)
        if not bmp:
            return False
        gdiplus.GdipDrawImageRect(self._gfx, bmp, x, y, size, size)
        return True

    def draw_text(
        self, text: str, x: float, y: float, w: float, h: float, color: int,
        size_px: float = 13.0, bold: bool = False, mono: bool = False, align: int = 0, valign: int = 1,
    ) -> None:
        if not text:
            return
        font = self._canvas._get_font("Consolas" if mono else "Segoe UI", size_px, bold)
        fmt = ctypes.c_void_p(0)
        gdiplus.GdipCreateStringFormat(0, 0, ctypes.byref(fmt))
        gdiplus.GdipSetStringFormatAlign(fmt, align)
        gdiplus.GdipSetStringFormatLineAlign(fmt, valign)
        gdiplus.GdipSetStringFormatTrimming(fmt, STRING_TRIMMING_ELLIPSIS_CHAR)
        gdiplus.GdipSetStringFormatFlags(fmt, STRING_FORMAT_NO_WRAP)
        brush = ctypes.c_void_p(0)
        gdiplus.GdipCreateSolidFill(color, ctypes.byref(brush))
        rect = RectF(x, y, max(1.0, w), max(1.0, h))
        gdiplus.GdipDrawString(self._gfx, text, len(text), font, ctypes.byref(rect), fmt, brush)
        gdiplus.GdipDeleteBrush(brush)
        gdiplus.GdipDeleteStringFormat(fmt)
