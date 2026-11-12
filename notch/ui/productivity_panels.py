from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from typing import Callable

from notch.modules.calendar_mod import AgendaItem
from notch.modules.launcher import AppShortcut
from notch.modules.tasks import TaskFilter, TaskItem
from notch.modules.timer import TimerState
from notch.ui.gdiplus import GdiPlusPainter
from notch.ui.theme import (
    AMBER, EMERALD, ROSE, SKY, SURFACE, TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY, VIOLET,
    blend_alpha, draw_card, draw_icon, draw_icon_button, draw_pill_button, draw_text_input,
)
from notch.ui.widgets import Rect, TextInputState, UiInteractionState


@dataclass
class ProductivityState:
    task_filter: TaskFilter = TaskFilter.TODAY
    tasks: list[TaskItem] = field(default_factory=list)
    task_input: TextInputState = field(default_factory=lambda: TextInputState(placeholder="Add task... (e.g. Call Alex 18:30)"))
    agenda: list[AgendaItem] = field(default_factory=list)
    timer: TimerState = field(default_factory=TimerState)
    shortcuts: list[AppShortcut] = field(default_factory=list)
    shortcut_input: TextInputState = field(default_factory=lambda: TextInputState(placeholder="Pin URL or path (e.g. https://github.com)"))


@dataclass
class ProductivityCallbacks:
    on_set_task_filter: Callable[[TaskFilter], None]
    on_add_task: Callable[[], None]
    on_toggle_task: Callable[[int], None]
    on_delete_task: Callable[[int], None]
    on_move_task: Callable[[int, int], None]
    on_import_ics: Callable[[], None]
    on_timer_toggle: Callable[[], None]
    on_timer_reset: Callable[[], None]
    on_timer_preset: Callable[[int, str], None]
    on_launch_shortcut: Callable[[str], None]
    on_remove_shortcut: Callable[[int], None]
    on_pin_file_dialog: Callable[[], None]
    on_pin_text_shortcut: Callable[[], None]


def draw_tasks_panel(painter: GdiPlusPainter, interaction: UiInteractionState, state: ProductivityState, s: float, x: float, y: float, w: float, cb: ProductivityCallbacks) -> None:
    fx = x
    for tf in TaskFilter:
        fw = 74.0 * s
        draw_pill_button(painter, interaction, f"tf_{tf.name}", fx, y, fw, 24.0 * s, tf.value, lambda f=tf: cb.on_set_task_filter(f), active=state.task_filter == tf, accent=EMERALD, size_px=10.5 * s, radius=8.0 * s)
        fx += fw + 6.0 * s
    painter.draw_text(f"{len(state.tasks)} items", x + w - 80.0 * s, y, 80.0 * s, 24.0 * s, TEXT_DIM, size_px=10.0 * s, align=2)

    inp_y, inp_w = y + 32.0 * s, w - 74.0 * s
    draw_text_input(painter, interaction, "task_input", x, inp_y, inp_w, 30.0 * s, state.task_input, state.task_input.focused, state.task_input.placeholder, enter_hint="↵")
    draw_pill_button(painter, interaction, "task_add", x + inp_w + 8.0 * s, inp_y, 66.0 * s, 30.0 * s, "Add", cb.on_add_task, active=bool(state.task_input.text), accent=EMERALD, icon="plus", radius=9.0 * s)

    list_y, list_h = inp_y + 38.0 * s, 160.0 * s
    draw_card(painter, x, list_y, w, list_h, radius=12.0 * s)
    if not state.tasks:
        painter.draw_text("All clear — add a task or time reminder above", x + 12.0 * s, list_y + 66.0 * s, w - 24.0 * s, 24.0 * s, TEXT_MUTED, size_px=11.0 * s, align=1)
        return
    for idx, item in enumerate(state.tasks[:5]):
        ry = list_y + 7.0 * s + idx * 30.0 * s
        draw_icon_button(painter, interaction, f"task_chk_{item.id}", x + 20.0 * s, ry + 13.0 * s, 9.5 * s, "check" if item.done else "spark", lambda tid=item.id: cb.on_toggle_task(tid), fill=blend_alpha(EMERALD, 65) if item.done else SURFACE, icon_color=EMERALD if item.done else TEXT_DIM, accent=EMERALD, active=item.done)
        due_str = f" · {datetime.fromtimestamp(item.due_at).strftime('%H:%M')}" if item.due_at else ""
        painter.draw_text(f"{item.title}{due_str}", x + 36.0 * s, ry + 2.0 * s, w - 134.0 * s, 22.0 * s, TEXT_DIM if item.done else TEXT_PRIMARY, size_px=11.2 * s, bold=not item.done)
        draw_pill_button(painter, interaction, f"task_up_{item.id}", x + w - 90.0 * s, ry + 3.0 * s, 24.0 * s, 20.0 * s, "↑", lambda tid=item.id: cb.on_move_task(tid, -1), size_px=10.0 * s, radius=6.0 * s)
        draw_pill_button(painter, interaction, f"task_dn_{item.id}", x + w - 62.0 * s, ry + 3.0 * s, 24.0 * s, 20.0 * s, "↓", lambda tid=item.id: cb.on_move_task(tid, 1), size_px=10.0 * s, radius=6.0 * s)
        draw_icon_button(painter, interaction, f"task_del_{item.id}", x + w - 20.0 * s, ry + 13.0 * s, 10.0 * s, "close", lambda tid=item.id: cb.on_delete_task(tid), fill=SURFACE, icon_color=ROSE)


def draw_calendar_panel(painter: GdiPlusPainter, interaction: UiInteractionState, state: ProductivityState, s: float, x: float, y: float, w: float, cb: ProductivityCallbacks) -> None:
    painter.draw_text(datetime.now().strftime("%A · %b %d"), x + 2.0 * s, y, w - 120.0 * s, 26.0 * s, TEXT_PRIMARY, size_px=13.5 * s, bold=True)
    draw_pill_button(painter, interaction, "cal_ics", x + w - 108.0 * s, y, 108.0 * s, 26.0 * s, "Import .ics", cb.on_import_ics, active=True, accent=VIOLET, icon="plus", size_px=10.5 * s, radius=8.0 * s)
    list_y, list_h = y + 34.0 * s, 194.0 * s
    draw_card(painter, x, list_y, w, list_h, radius=12.0 * s)
    if not state.agenda:
        painter.draw_text("No scheduled events or tasks today", x + 16.0 * s, list_y + 78.0 * s, w - 32.0 * s, 24.0 * s, TEXT_MUTED, size_px=11.5 * s, align=1)
        return
    for idx, ev in enumerate(state.agenda[:5]):
        ry = list_y + 10.0 * s + idx * 35.0 * s
        when = datetime.fromtimestamp(ev.starts_at).strftime("%a %H:%M")
        painter.fill_round_rect(x + 10.0 * s, ry + 3.0 * s, 80.0 * s, 24.0 * s, 7.0 * s, blend_alpha(VIOLET, 36))
        painter.draw_text(when, x + 10.0 * s, ry + 3.0 * s, 80.0 * s, 24.0 * s, VIOLET, size_px=10.0 * s, bold=True, mono=True, align=1)
        painter.draw_text(ev.summary, x + 98.0 * s, ry + 3.0 * s, w - 166.0 * s, 24.0 * s, TEXT_PRIMARY, size_px=11.5 * s, bold=True)
        painter.draw_text(ev.source.upper(), x + w - 60.0 * s, ry + 3.0 * s, 48.0 * s, 24.0 * s, SKY if ev.source == "ics" else EMERALD, size_px=9.5 * s, bold=True, align=2)


def draw_timer_panel(painter: GdiPlusPainter, interaction: UiInteractionState, state: ProductivityState, s: float, x: float, y: float, w: float, cb: ProductivityCallbacks) -> None:
    tmr = state.timer
    dial_w, panel_h = w * 0.45, 226.0 * s
    accent = EMERALD if tmr.completed else AMBER
    draw_card(painter, x, y, dial_w, panel_h, radius=14.0 * s, accent=accent if (tmr.running or tmr.completed) else 0, elevated=True)
    ring_sz = 136.0 * s
    rx, ry = x + (dial_w - ring_sz) * 0.5, y + 26.0 * s
    painter.draw_arc(rx, ry, ring_sz, ring_sz, 0.0, 360.0, SURFACE, 7.0 * s)
    painter.draw_arc(rx, ry, ring_sz, ring_sz, -90.0, 360.0 * tmr.progress_fraction(), accent, 7.5 * s)
    painter.draw_text(tmr.formatted_remaining(), rx, ry + 36.0 * s, ring_sz, 40.0 * s, TEXT_PRIMARY, size_px=26.0 * s, bold=True, mono=True, align=1)
    painter.draw_text(tmr.label, rx + 8.0 * s, ry + 80.0 * s, ring_sz - 16.0 * s, 20.0 * s, accent if tmr.running else TEXT_MUTED, size_px=10.5 * s, bold=True, align=1)
    painter.draw_text("Completed!" if tmr.completed else ("Running" if tmr.running else "Ready"), x + 12.0 * s, y + panel_h - 30.0 * s, dial_w - 24.0 * s, 20.0 * s, accent if (tmr.running or tmr.completed) else TEXT_DIM, size_px=10.5 * s, bold=True, align=1)

    cx, cw = x + dial_w + 12.0 * s, w - dial_w - 12.0 * s
    draw_pill_button(painter, interaction, "tmr_toggle", cx, y + 6.0 * s, cw * 0.62, 36.0 * s, "Pause" if tmr.running else "Start Timer", cb.on_timer_toggle, active=True, accent=EMERALD if not tmr.running else AMBER, icon="pause" if tmr.running else "play", radius=11.0 * s)
    draw_pill_button(painter, interaction, "tmr_reset", cx + cw * 0.66, y + 6.0 * s, cw * 0.34, 36.0 * s, "Reset", cb.on_timer_reset, radius=11.0 * s)
    painter.draw_text("PRESETS", cx + 2.0 * s, y + 58.0 * s, cw, 16.0 * s, TEXT_DIM, size_px=9.5 * s, bold=True)
    pw, ph = (cw - 8.0 * s) * 0.5, 52.0 * s
    for idx, (label, secs) in enumerate([("5m Break", 300), ("15m Sprint", 900), ("25m Focus", 1500), ("45m Deep", 2700)]):
        col, row = idx % 2, idx // 2
        draw_pill_button(painter, interaction, f"tmr_p_{secs}", cx + col * (pw + 8.0 * s), y + 80.0 * s + row * (ph + 8.0 * s), pw, ph, label, lambda sc=secs, lb=label: cb.on_timer_preset(sc, lb), active=tmr.total_secs == secs, accent=AMBER, size_px=11.5 * s, radius=11.0 * s)


def draw_apps_panel(painter: GdiPlusPainter, interaction: UiInteractionState, state: ProductivityState, s: float, x: float, y: float, w: float, cb: ProductivityCallbacks) -> None:
    inp_w = w - 158.0 * s
    draw_text_input(painter, interaction, "app_url_input", x, y, inp_w, 30.0 * s, state.shortcut_input, state.shortcut_input.focused, state.shortcut_input.placeholder, enter_hint="↵")
    draw_pill_button(painter, interaction, "app_pin_url", x + inp_w + 6.0 * s, y, 72.0 * s, 30.0 * s, "Pin URL", cb.on_pin_text_shortcut, active=bool(state.shortcut_input.text), accent=SKY, size_px=10.5 * s, radius=9.0 * s)
    draw_pill_button(painter, interaction, "app_pin_file", x + inp_w + 84.0 * s, y, 74.0 * s, 30.0 * s, "App", cb.on_pin_file_dialog, icon="plus", size_px=10.5 * s, radius=9.0 * s)
    grid_y = y + 40.0 * s
    if not state.shortcuts:
        draw_card(painter, x, grid_y, w, 186.0 * s, radius=12.0 * s)
        painter.draw_text("Pin apps, folders, or URLs for 1-click launching", x + 16.0 * s, grid_y + 78.0 * s, w - 32.0 * s, 24.0 * s, TEXT_MUTED, size_px=11.5 * s, align=1)
        return
    cols, gap = 4, 8.0 * s
    tile_w, tile_h = (w - gap * (cols - 1)) / cols, 88.0 * s
    for idx, sc in enumerate(state.shortcuts[:8]):
        tx, ty = x + (idx % cols) * (tile_w + gap), grid_y + (idx // cols) * (tile_h + gap)
        hovered = interaction.register_button(f"app_launch_{sc.id}", Rect(tx, ty, tile_w, tile_h), lambda tgt=sc.target: cb.on_launch_shortcut(tgt))
        draw_card(painter, tx, ty, tile_w, tile_h, radius=11.0 * s, elevated=hovered)
        ico_sz = 32.0 * s
        ix, iy = tx + (tile_w - ico_sz) * 0.5, ty + 14.0 * s
        if not painter.draw_target_icon(sc.target, ix, iy, ico_sz):
            painter.fill_round_rect(ix, iy, ico_sz, ico_sz, 8.0 * s, blend_alpha(SKY, 42))
            draw_icon(painter, "folder", ix + ico_sz * 0.5, iy + ico_sz * 0.5, 16.0 * s, SKY)
        painter.draw_text(sc.name, tx + 6.0 * s, ty + 54.0 * s, tile_w - 12.0 * s, 20.0 * s, TEXT_PRIMARY, size_px=10.5 * s, bold=True, align=1)
        draw_icon_button(painter, interaction, f"app_rm_{sc.id}", tx + tile_w - 13.0 * s, ty + 13.0 * s, 8.5 * s, "close", lambda sid=sc.id: cb.on_remove_shortcut(sid), fill=SURFACE, icon_color=TEXT_MUTED if not hovered else ROSE)
