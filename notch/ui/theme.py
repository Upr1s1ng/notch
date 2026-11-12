from __future__ import annotations

from typing import Callable, Optional

from notch.ui.gdiplus import GdiPlusPainter, argb
from notch.ui.widgets import Rect, TextInputState, UiInteractionState

BG_TOP = argb(246, 17, 19, 26)
BG_BOTTOM = argb(245, 9, 10, 14)
RIM_INNER = argb(46, 255, 255, 255)
SPECULAR_TOP = argb(72, 255, 255, 255)

CARD_BG = argb(17, 255, 255, 255)
CARD_BG_ELEVATED = argb(24, 255, 255, 255)
CARD_BORDER = argb(28, 255, 255, 255)
SURFACE = argb(24, 255, 255, 255)
SURFACE_HOVER = argb(44, 255, 255, 255)
SURFACE_PRESS = argb(62, 255, 255, 255)
TAB_ACTIVE = argb(48, 255, 255, 255)
DIVIDER = argb(28, 255, 255, 255)

TEXT_PRIMARY = argb(255, 248, 250, 252)
TEXT_SECONDARY = argb(220, 203, 213, 225)
TEXT_MUTED = argb(190, 148, 163, 184)
TEXT_DIM = argb(145, 110, 125, 148)

EMERALD = argb(255, 52, 211, 153)
AMBER = argb(255, 251, 191, 36)
SKY = argb(255, 125, 211, 252)
ROSE = argb(255, 251, 113, 133)
VIOLET = argb(255, 167, 139, 250)


def blend_alpha(argb_color: int, alpha: int) -> int:
    return ((max(0, min(255, alpha)) & 0xFF) << 24) | (argb_color & 0x00FFFFFF)


def draw_island_shell(painter: GdiPlusPainter, x: float, y: float, w: float, h: float, radius: float, glow_color: int = 0) -> None:
    painter.fill_round_rect(x - 1.0, y - 0.5, w + 2.0, h + 2.5, radius + 1.0, argb(72, 0, 0, 0))
    painter.fill_round_rect_gradient(x, y, w, h, radius, BG_TOP, BG_BOTTOM)
    if glow_color:
        painter.fill_round_rect_gradient(
            x + 1.0, y + 1.0, w - 2.0, min(h - 2.0, max(28.0, h * 0.45)),
            max(4.0, radius - 1.0), blend_alpha(glow_color, 34), 0,
        )
    painter.stroke_round_rect(x, y, w, h, radius, RIM_INNER, thickness=1.0)
    rim_inset = max(12.0, min(radius * 0.85, w * 0.14))
    if w > rim_inset * 2.0 + 8.0:
        painter.draw_line(x + rim_inset, y + 0.75, x + w - rim_inset, y + 0.75, SPECULAR_TOP, width=1.0)


def draw_card(painter: GdiPlusPainter, x: float, y: float, w: float, h: float, radius: float = 12.0, accent: int = 0, elevated: bool = False) -> None:
    painter.fill_round_rect(x, y, w, h, radius, CARD_BG_ELEVATED if elevated else CARD_BG)
    if accent:
        painter.fill_round_rect_gradient(x + 1.0, y + 1.0, w - 2.0, min(h - 2.0, 26.0), max(4.0, radius - 1.0), blend_alpha(accent, 28), 0)
    painter.stroke_round_rect(x, y, w, h, radius, CARD_BORDER, thickness=1.0)


def draw_icon(painter: GdiPlusPainter, name: str, cx: float, cy: float, size: float, color: int) -> None:
    r = size * 0.42
    w = max(1.3, size * 0.11)
    if name == "play":
        ox = cx + r * 0.1
        painter.fill_polygon([(ox - r * 0.55, cy - r * 0.72), (ox + r * 0.78, cy), (ox - r * 0.55, cy + r * 0.72)], color)
    elif name == "pause":
        bw, bh, gap = max(2.2, r * 0.42), r * 1.35, max(2.0, r * 0.34)
        painter.fill_round_rect(cx - gap * 0.5 - bw, cy - bh * 0.5, bw, bh, 1.2, color)
        painter.fill_round_rect(cx + gap * 0.5, cy - bh * 0.5, bw, bh, 1.2, color)
    elif name == "prev":
        painter.fill_round_rect(cx - r * 0.75, cy - r * 0.65, 2.0, r * 1.3, 1.0, color)
        painter.fill_polygon([(cx - r * 0.4, cy), (cx + r * 0.65, cy - r * 0.65), (cx + r * 0.65, cy + r * 0.65)], color)
    elif name == "next":
        painter.fill_polygon([(cx - r * 0.65, cy - r * 0.65), (cx + r * 0.4, cy), (cx - r * 0.65, cy + r * 0.65)], color)
        painter.fill_round_rect(cx + r * 0.55, cy - r * 0.65, 2.0, r * 1.3, 1.0, color)
    elif name == "search":
        gr, gx, gy = r * 0.58, cx - r * 0.18, cy - r * 0.18
        painter.stroke_circle(gx, gy, gr, color, width=w)
        painter.draw_line(gx + gr * 0.72, gy + gr * 0.72, cx + r * 0.82, cy + r * 0.82, color, width=w * 1.15)
    elif name in ("mic", "mic_off"):
        painter.stroke_round_rect(cx - r * 0.34, cy - r * 0.78, r * 0.68, r * 1.02, r * 0.34, color, thickness=w)
        painter.draw_arc(cx - r * 0.62, cy - r * 0.35, r * 1.24, r * 0.92, 0.0, 180.0, color, width=w)
        painter.draw_line(cx, cy + r * 0.57, cx, cy + r * 0.86, color, width=w)
        painter.draw_line(cx - r * 0.35, cy + r * 0.86, cx + r * 0.35, cy + r * 0.86, color, width=w)
        if name == "mic_off":
            painter.draw_line(cx - r * 0.75, cy - r * 0.7, cx + r * 0.75, cy + r * 0.75, ROSE, width=w * 1.15)
    elif name in ("volume", "volume_mute"):
        painter.fill_polygon(
            [(cx - r * 0.78, cy - r * 0.28), (cx - r * 0.36, cy - r * 0.28), (cx + r * 0.12, cy - r * 0.68),
             (cx + r * 0.12, cy + r * 0.68), (cx - r * 0.36, cy + r * 0.28), (cx - r * 0.78, cy + r * 0.28)],
            color,
        )
        if name == "volume":
            painter.draw_arc(cx - r * 0.05, cy - r * 0.45, r * 0.65, r * 0.9, -52.0, 104.0, color, width=w)
            painter.draw_arc(cx - r * 0.05, cy - r * 0.72, r * 1.0, r * 1.44, -48.0, 96.0, color, width=w)
        else:
            painter.draw_line(cx + r * 0.35, cy - r * 0.32, cx + r * 0.82, cy + r * 0.32, ROSE, width=w * 1.1)
            painter.draw_line(cx + r * 0.82, cy - r * 0.32, cx + r * 0.35, cy + r * 0.32, ROSE, width=w * 1.1)
    elif name == "sun":
        painter.stroke_circle(cx, cy, r * 0.42, color, width=w)
        for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1), (0.7, 0.7), (-0.7, -0.7), (0.7, -0.7), (-0.7, 0.7)]:
            painter.draw_line(cx + dx * r * 0.68, cy + dy * r * 0.68, cx + dx * r * 0.96, cy + dy * r * 0.96, color, width=w)
    elif name == "clock":
        painter.stroke_circle(cx, cy, r * 0.78, color, width=w)
        painter.draw_polyline([(cx, cy - r * 0.45), (cx, cy), (cx + r * 0.38, cy + r * 0.22)], color, width=w)
    elif name == "check":
        painter.draw_polyline([(cx - r * 0.58, cy + r * 0.05), (cx - r * 0.14, cy + r * 0.48), (cx + r * 0.62, cy - r * 0.45)], color, width=w * 1.25)
    elif name == "plus":
        painter.draw_line(cx - r * 0.62, cy, cx + r * 0.62, cy, color, width=w * 1.1)
        painter.draw_line(cx, cy - r * 0.62, cx, cy + r * 0.62, color, width=w * 1.1)
    elif name == "close":
        painter.draw_line(cx - r * 0.52, cy - r * 0.52, cx + r * 0.52, cy + r * 0.52, color, width=w * 1.1)
        painter.draw_line(cx + r * 0.52, cy - r * 0.52, cx - r * 0.52, cy + r * 0.52, color, width=w * 1.1)
    elif name == "folder":
        painter.fill_round_rect(cx - r * 0.78, cy - r * 0.68, r * 0.72, r * 0.35, 1.5, color)
        painter.stroke_round_rect(cx - r * 0.82, cy - r * 0.46, r * 1.64, r * 1.12, 2.5, color, thickness=w)
    elif name == "grip":
        dot_r = max(1.0, size * 0.08)
        for row in (-0.48, 0.0, 0.48):
            for col in (-0.28, 0.28):
                painter.fill_circle(cx + col * r, cy + row * r, dot_r, color)
    elif name == "spark":
        painter.fill_polygon(
            [(cx, cy - r * 0.85), (cx + r * 0.26, cy - r * 0.26), (cx + r * 0.85, cy), (cx + r * 0.26, cy + r * 0.26),
             (cx, cy + r * 0.85), (cx - r * 0.26, cy + r * 0.26), (cx - r * 0.85, cy), (cx - r * 0.26, cy - r * 0.26)],
            color,
        )


def draw_pill_button(
    painter: GdiPlusPainter, interaction: UiInteractionState, btn_id: str,
    x: float, y: float, w: float, h: float, label: str, on_click: Callable[[], None],
    *, active: bool = False, accent: int = 0, icon: Optional[str] = None,
    size_px: float = 11.0, bold: bool = True, radius: Optional[float] = None, text_color: Optional[int] = None,
) -> None:
    hovered = interaction.register_button(btn_id, Rect(x, y, w, h), on_click)
    pressed = interaction.is_pressed(btn_id)
    inset = 0.8 if pressed else 0.0
    dx, dy, dw, dh = x + inset, y + inset, max(4.0, w - inset * 2.0), max(4.0, h - inset * 2.0)
    r = dh * 0.5 if radius is None else min(radius, dh * 0.5)
    if pressed:
        bg = blend_alpha(accent, 95) if accent else SURFACE_PRESS
    elif active:
        bg = blend_alpha(accent, 58) if accent else TAB_ACTIVE
    elif hovered:
        bg = blend_alpha(accent, 42) if accent else SURFACE_HOVER
    else:
        bg = blend_alpha(accent, 22) if accent else SURFACE
    painter.fill_round_rect(dx, dy, dw, dh, r, bg)
    border = blend_alpha(accent, 110 if active else 65) if (active and accent) else (argb(56, 255, 255, 255) if hovered else CARD_BORDER)
    painter.stroke_round_rect(dx, dy, dw, dh, r, border, thickness=1.0)
    fg = text_color if text_color is not None else (accent if (active and accent) else TEXT_PRIMARY)
    if icon and label:
        draw_icon(painter, icon, dx + 12.0, dy + dh * 0.5, min(13.0, dh * 0.56), fg)
        painter.draw_text(label, dx + 21.0, dy, dw - 27.0, dh, fg, size_px=size_px, bold=bold, align=0)
    elif icon:
        draw_icon(painter, icon, dx + dw * 0.5, dy + dh * 0.5, min(14.0, min(dw, dh) * 0.62), fg)
    else:
        painter.draw_text(label, dx + 6.0, dy, max(1.0, dw - 12.0), dh, fg, size_px=size_px, bold=bold, align=1)


def draw_icon_button(
    painter: GdiPlusPainter, interaction: UiInteractionState, btn_id: str,
    cx: float, cy: float, radius: float, icon: str, on_click: Callable[[], None],
    *, fill: int = 0, icon_color: int = TEXT_PRIMARY, accent: int = 0, active: bool = False,
) -> None:
    d = radius * 2.0
    hovered = interaction.register_button(btn_id, Rect(cx - radius, cy - radius, d, d), on_click)
    pressed = interaction.is_pressed(btn_id)
    dr = radius - (1.0 if pressed else 0.0)
    bg = fill
    if pressed:
        bg = blend_alpha(accent, 110) if accent else SURFACE_PRESS
    elif active and accent:
        bg = blend_alpha(accent, 70)
    elif hovered:
        bg = blend_alpha(fill, 235) if fill else SURFACE_HOVER
    elif not bg:
        bg = SURFACE
    painter.fill_circle(cx, cy, dr, bg)
    if hovered or active:
        painter.stroke_circle(cx, cy, dr, blend_alpha(accent or TEXT_PRIMARY, 90), width=1.0)
    draw_icon(painter, icon, cx, cy, dr * 1.15, icon_color)


def draw_slider(
    painter: GdiPlusPainter, interaction: UiInteractionState, slider_id: str,
    x: float, y: float, w: float, h: float, ratio: float, fill_color: int, on_change: Callable[[float], None],
) -> None:
    clamped = max(0.0, min(1.0, ratio))
    hovered = interaction.register_slider(slider_id, Rect(x, y, w, h), on_change)
    dragging = interaction.active_slider_id == slider_id
    th = 7.0 if (hovered or dragging) else 5.0
    ty = y + (h - th) * 0.5
    painter.fill_round_rect(x, ty, w, th, th * 0.5, argb(34, 255, 255, 255))
    if clamped > 0.0:
        painter.fill_round_rect(x, ty, max(th, clamped * w), th, th * 0.5, fill_color)
    kx = max(x + 6.0, min(x + w - 6.0, x + clamped * w))
    kr = 7.0 if dragging else (6.0 if hovered else 4.8)
    if dragging or hovered:
        painter.fill_circle(kx, y + h * 0.5, kr + 3.0, blend_alpha(fill_color, 55))
    painter.fill_circle(kx, y + h * 0.5, kr, TEXT_PRIMARY)


def draw_text_input(
    painter: GdiPlusPainter, interaction: UiInteractionState, input_id: str,
    x: float, y: float, w: float, h: float, input_state: TextInputState, is_focused: bool, placeholder: str,
    *, icon: Optional[str] = None, enter_hint: Optional[str] = None,
) -> None:
    interaction.register_input(input_id, Rect(x, y, w, h))
    bg = argb(46, 255, 255, 255) if is_focused else argb(20, 255, 255, 255)
    border = blend_alpha(SKY, 210) if is_focused else CARD_BORDER
    r = min(11.0, h * 0.45)
    painter.fill_round_rect(x, y, w, h, r, bg)
    painter.stroke_round_rect(x, y, w, h, r, border, thickness=1.5 if is_focused else 1.0)
    left_pad = 10.0
    if icon:
        draw_icon(painter, icon, x + 15.0, y + h * 0.5, 13.0, SKY if is_focused else TEXT_MUTED)
        left_pad = 29.0
    right_pad = 10.0
    if enter_hint and is_focused:
        bw = 24.0
        painter.fill_round_rect(x + w - bw - 6.0, y + 6.0, bw, h - 12.0, 5.0, argb(36, 125, 211, 252))
        painter.draw_text(enter_hint, x + w - bw - 6.0, y + 6.0, bw, h - 12.0, SKY, size_px=10.0, bold=True, align=1)
        right_pad = 36.0
    text = input_state.text
    display = (text[: input_state.cursor] + "|" + text[input_state.cursor :]) if (is_focused and text) else ("|" if is_focused else (text or placeholder))
    fg = TEXT_PRIMARY if (is_focused or text) else TEXT_MUTED
    painter.draw_text(display, x + left_pad, y, max(10.0, w - left_pad - right_pad), h, fg, size_px=11.5)
