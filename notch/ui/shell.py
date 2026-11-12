from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Callable

from notch.core.config import AppConfig
from notch.core.state_machine import ExpandedTab, NotchState, NotchStateMachine
from notch.modules.discord_ipc import DiscordVoiceState
from notch.ui.gdiplus import GdiPlusPainter, argb
from notch.ui.media_audio_panels import MediaAudioCallbacks, MediaAudioState, draw_audio_panel, draw_media_panel
from notch.ui.productivity_panels import ProductivityCallbacks, ProductivityState, draw_apps_panel, draw_calendar_panel, draw_tasks_panel, draw_timer_panel
from notch.ui.rack_panel import RackCallbacks, RackUiState, draw_rack_panel
from notch.ui.theme import (
    AMBER, BG_TOP, DIVIDER, EMERALD, ROSE, SKY, SURFACE, SURFACE_HOVER, TAB_ACTIVE,
    TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, VIOLET,
    blend_alpha, draw_card, draw_icon, draw_icon_button, draw_island_shell, draw_pill_button, draw_text_input,
)
from notch.ui.widgets import Rect, TextInputState, UiInteractionState

COLOR_PILL_BG = BG_TOP


@dataclass
class ShellCallbacks:
    on_toggle_expand: Callable[[], None]
    on_collapse: Callable[[], None]
    on_hide_manual: Callable[[], None]
    on_trigger_peek: Callable[[], None]
    on_submit_search: Callable[[], None]
    on_select_tab: Callable[[ExpandedTab], None]
    on_discord_mute: Callable[[], None]
    on_discord_deafen: Callable[[], None]
    on_discord_disconnect: Callable[[], None]
    on_discord_toggle_hotkey_preset: Callable[[], None]
    media_audio: MediaAudioCallbacks
    productivity: ProductivityCallbacks
    rack: RackCallbacks
    on_toggle_capture_privacy: Callable[[], None] | None = None


@dataclass
class ShellUiState:
    search_input: TextInputState = field(default_factory=lambda: TextInputState(placeholder="Search Google... (Ctrl+Alt+Space)"))
    focused_input_id: str | None = None
    capture_excluded: bool = False
    discord: DiscordVoiceState = field(default_factory=DiscordVoiceState)
    interaction: UiInteractionState = field(default_factory=UiInteractionState)
    media_audio: MediaAudioState = field(default_factory=MediaAudioState)
    productivity: ProductivityState = field(default_factory=ProductivityState)
    rack: RackUiState = field(default_factory=RackUiState)

    def active_input(self) -> TextInputState | None:
        return {"google_search": self.search_input, "task_input": self.productivity.task_input, "app_url_input": self.productivity.shortcut_input}.get(self.focused_input_id or "")

    def set_focused_input(self, input_id: str | None) -> None:
        self.focused_input_id = input_id
        self.search_input.focused = input_id == "google_search"
        self.productivity.task_input.focused = input_id == "task_input"
        self.productivity.shortcut_input.focused = input_id == "app_url_input"


def _resolve_glow(painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState) -> int:
    media = ui_state.media_audio.media
    if ui_state.discord.in_call:
        return ROSE if ui_state.discord.muted else EMERALD
    if media.has_session and media.is_playing:
        return painter.get_image_dominant_color(media.thumbnail_bytes) or SKY
    if ui_state.productivity.timer.running:
        return AMBER
    if sm.state == NotchState.PEEK:
        return EMERALD
    return 0


def draw_notch_shell(
    painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState,
    config: AppConfig, dpi_scale: float, width_px: int, height_px: int, callbacks: ShellCallbacks,
) -> None:
    ui_state.interaction.begin_frame()
    pad = 2.0 * dpi_scale
    w, h = width_px - pad * 2.0, height_px - pad * 2.0
    radius = min(sm.corner_radius.value * dpi_scale, h * 0.5)
    glow = _resolve_glow(painter, sm, ui_state)
    draw_island_shell(painter, pad, pad, w, h, radius, glow_color=glow)

    if sm.state == NotchState.COLLAPSED:
        _draw_collapsed(painter, sm, ui_state, dpi_scale, pad, w, h, callbacks)
    elif sm.state == NotchState.PEEK:
        _draw_peek(painter, sm, ui_state, dpi_scale, pad, w, h, callbacks, glow)
    elif sm.state == NotchState.EXPANDED:
        _draw_expanded(painter, sm, ui_state, config, dpi_scale, pad, w, h, callbacks)


def _draw_collapsed(painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState, s: float, pad: float, w: float, h: float, callbacks: ShellCallbacks) -> None:
    ui_state.interaction.register_button("collapsed_pill", Rect(pad, pad, w, h), callbacks.on_toggle_expand)
    media, dc, tmr = ui_state.media_audio.media, ui_state.discord, ui_state.productivity.timer
    rack_count = len(ui_state.rack.items)
    clock_str = time.strftime("%H:%M")
    clock_w = 48.0 * s
    clock_x = pad + w - clock_w - 10.0 * s

    if rack_count > 0 and not dc.in_call:
        bw = 24.0 * s
        bx = clock_x - bw - 4.0 * s
        painter.fill_round_rect(bx, pad + 8.0 * s, bw, h - 16.0 * s, 6.0 * s, blend_alpha(SKY, 42))
        painter.draw_text(f"{rack_count}F", bx, pad, bw, h, SKY, size_px=9.5 * s, bold=True, mono=True, align=1)
        label_right = bx - 4.0 * s
    else:
        label_right = clock_x - 4.0 * s

    if dc.in_call:
        dot_col = ROSE if dc.muted else EMERALD
        painter.fill_circle(pad + 15.0 * s, pad + h * 0.5, 4.0 * s, dot_col)
        mute_r = 11.0 * s
        mute_cx = clock_x - mute_r - 6.0 * s
        draw_icon_button(painter, ui_state.interaction, "pill_dc_mute", mute_cx, pad + h * 0.5, mute_r, "mic_off" if dc.muted else "mic", callbacks.on_discord_mute, fill=blend_alpha(dot_col, 48), icon_color=ROSE if dc.muted else TEXT_PRIMARY, accent=dot_col, active=True)
        painter.draw_text("Muted" if dc.muted else "Discord Voice", pad + 26.0 * s, pad, max(20.0, mute_cx - mute_r - (pad + 30.0 * s)), h, TEXT_PRIMARY, size_px=11.0 * s, bold=True)
    elif tmr.running or tmr.remaining_secs < tmr.total_secs:
        ring_sz = 15.0 * s
        rx, ry = pad + 11.0 * s, pad + (h - ring_sz) * 0.5
        painter.draw_arc(rx, ry, ring_sz, ring_sz, 0.0, 360.0, SURFACE_HOVER, 2.0 * s)
        painter.draw_arc(rx, ry, ring_sz, ring_sz, -90.0, 360.0 * tmr.progress_fraction(), AMBER, 2.2 * s)
        painter.draw_text(tmr.formatted_remaining(), pad + 32.0 * s, pad, max(20.0, label_right - (pad + 32.0 * s)), h, AMBER, size_px=11.5 * s, bold=True, mono=True)
    elif media.has_session and media.title:
        art_sz, ax, ay = 20.0 * s, pad + 9.0 * s, pad + (h - 20.0 * s) * 0.5
        accent = painter.draw_image_bytes(media.thumbnail_bytes, ax, ay, art_sz, art_sz, radius=5.0 * s)
        if not media.thumbnail_bytes:
            painter.fill_round_rect(ax, ay, art_sz, art_sz, 5.0 * s, blend_alpha(SKY, 48))
            draw_icon(painter, "play" if media.is_playing else "pause", ax + art_sz * 0.5, pad + h * 0.5, 10.0 * s, SKY)
            accent = EMERALD if media.is_playing else TEXT_MUTED
        if media.is_playing:
            bar_x = label_right - 14.0 * s
            for idx, bh in enumerate((6.0 * s, 11.0 * s, 7.5 * s)):
                painter.fill_round_rect(bar_x + idx * 4.0 * s, pad + h * 0.5 - bh * 0.5, 2.4 * s, bh, 1.2 * s, accent)
            label_right = bar_x - 5.0 * s
        painter.draw_text(media.title, pad + 35.0 * s, pad, max(24.0, label_right - (pad + 35.0 * s)), h, TEXT_PRIMARY if media.is_playing else TEXT_SECONDARY, size_px=11.0 * s, bold=True)
    else:
        draw_icon(painter, "spark", pad + 16.0 * s, pad + h * 0.5, 11.0 * s, AMBER if sm.input_focused else EMERALD)
        painter.draw_text("Notch", pad + 29.0 * s, pad, max(20.0, label_right - (pad + 29.0 * s)), h, TEXT_PRIMARY, size_px=11.5 * s, bold=True)

    painter.draw_text(clock_str, clock_x, pad, clock_w, h, TEXT_MUTED, size_px=11.5 * s, bold=True, mono=True, align=2)


def _draw_peek(painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState, s: float, pad: float, w: float, h: float, callbacks: ShellCallbacks, glow: int) -> None:
    ui_state.interaction.register_button("peek_pill", Rect(pad, pad, w, h), callbacks.on_toggle_expand)
    if not sm.peek:
        return
    accent = glow or EMERALD
    progress = max(0.0, min(1.0, 1.0 - max(0.0, time.monotonic() - sm.peek.started_at) / max(0.01, sm.peek.duration_secs)))
    art_sz, ax, ay = 36.0 * s, pad + 12.0 * s, pad + 10.0 * s
    media = ui_state.media_audio.media
    if media.thumbnail_bytes and sm.peek.title == media.title:
        painter.draw_image_bytes(media.thumbnail_bytes, ax, ay, art_sz, art_sz, radius=9.0 * s)
    else:
        painter.fill_round_rect(ax, ay, art_sz, art_sz, 9.0 * s, blend_alpha(accent, 42))
        painter.stroke_round_rect(ax, ay, art_sz, art_sz, 9.0 * s, blend_alpha(accent, 90), 1.0 * s)
        draw_icon(painter, "spark", ax + art_sz * 0.5, ay + art_sz * 0.5, 16.0 * s, accent)
    tx, tw = ax + art_sz + 10.0 * s, w - (ax + art_sz + 10.0 * s - pad) - 56.0 * s
    painter.draw_text(sm.peek.title, tx, pad + 10.0 * s, tw, 18.0 * s, TEXT_PRIMARY, size_px=12.5 * s, bold=True)
    painter.draw_text(sm.peek.subtitle, tx, pad + 29.0 * s, tw, 16.0 * s, TEXT_SECONDARY, size_px=10.5 * s)
    painter.draw_text(time.strftime("%H:%M"), pad + w - 54.0 * s, pad + 11.0 * s, 42.0 * s, 16.0 * s, TEXT_DIM, size_px=10.5 * s, mono=True, align=2)
    bx, bw, by = pad + 14.0 * s, w - 28.0 * s, pad + h - 7.5 * s
    painter.fill_round_rect(bx, by, bw, 2.5 * s, 1.25 * s, SURFACE)
    painter.fill_round_rect(bx, by, max(2.5 * s, bw * progress), 2.5 * s, 1.25 * s, accent)


def _draw_expanded(painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState, config: AppConfig, s: float, pad: float, w: float, h: float, callbacks: ShellCallbacks) -> None:
    ix, iy, iw = pad + 14.0 * s, pad + 10.0 * s, w - 28.0 * s
    _draw_tab_bar(painter, sm, ui_state, s, ix, iy, iw, callbacks)
    div_y = iy + 30.0 * s
    painter.draw_line(ix, div_y, ix + iw, div_y, DIVIDER, 1.0 * s)
    cy = div_y + 9.0 * s
    tab = sm.active_tab
    if tab == ExpandedTab.OVERVIEW:
        _draw_overview_tab(painter, ui_state, config, s, ix, cy, iw, callbacks)
    elif tab == ExpandedTab.MEDIA:
        draw_media_panel(painter, ui_state.interaction, ui_state.media_audio, s, ix, cy, iw, callbacks.media_audio)
    elif tab == ExpandedTab.CONTROLS:
        draw_audio_panel(painter, ui_state.interaction, ui_state.media_audio, s, ix, cy, iw, callbacks.media_audio)
    elif tab == ExpandedTab.TASKS:
        draw_tasks_panel(painter, ui_state.interaction, ui_state.productivity, s, ix, cy, iw, callbacks.productivity)
    elif tab == ExpandedTab.CALENDAR:
        draw_calendar_panel(painter, ui_state.interaction, ui_state.productivity, s, ix, cy, iw, callbacks.productivity)
    elif tab == ExpandedTab.TIMER:
        draw_timer_panel(painter, ui_state.interaction, ui_state.productivity, s, ix, cy, iw, callbacks.productivity)
    elif tab == ExpandedTab.APPS:
        draw_apps_panel(painter, ui_state.interaction, ui_state.productivity, s, ix, cy, iw, callbacks.productivity)
    elif tab == ExpandedTab.RACK:
        draw_rack_panel(painter, ui_state.interaction, ui_state.rack, s, ix, cy, iw, callbacks.rack)


def _draw_tab_bar(painter: GdiPlusPainter, sm: NotchStateMachine, ui_state: ShellUiState, s: float, x: float, y: float, w: float, callbacks: ShellCallbacks) -> None:
    tabs = list(ExpandedTab)
    close_d, gap = 24.0 * s, 4.0 * s
    tab_w = (w - close_d - 8.0 * s - gap * (len(tabs) - 1)) / len(tabs)
    tab_h = 24.0 * s
    for idx, tab in enumerate(tabs):
        tx = x + idx * (tab_w + gap)
        btn_id = f"tab_{tab.id}"
        hovered = ui_state.interaction.register_button(btn_id, Rect(tx, y, tab_w, tab_h), lambda t=tab: callbacks.on_select_tab(t))
        pressed = ui_state.interaction.is_pressed(btn_id)
        selected = sm.active_tab == tab
        if selected:
            painter.fill_round_rect(tx, y, tab_w, tab_h, 8.0 * s, TAB_ACTIVE)
            painter.stroke_round_rect(tx, y, tab_w, tab_h, 8.0 * s, argb(48, 255, 255, 255), 1.0 * s)
        elif pressed:
            painter.fill_round_rect(tx, y, tab_w, tab_h, 8.0 * s, SURFACE_HOVER)
        elif hovered:
            painter.fill_round_rect(tx, y, tab_w, tab_h, 8.0 * s, SURFACE)
        painter.draw_text(tab.label, tx, y, tab_w, tab_h, TEXT_PRIMARY if selected else TEXT_MUTED, size_px=10.5 * s, bold=selected, align=1)
        dot_col = EMERALD if (tab == ExpandedTab.MEDIA and ui_state.media_audio.media.is_playing) else (AMBER if (tab == ExpandedTab.TIMER and ui_state.productivity.timer.running) else (SKY if (tab == ExpandedTab.RACK and ui_state.rack.items) else 0))
        if dot_col:
            painter.fill_circle(tx + tab_w - 6.0 * s, y + 6.0 * s, 2.2 * s, dot_col)
    draw_icon_button(painter, ui_state.interaction, "btn_close", x + w - close_d * 0.5, y + tab_h * 0.5, close_d * 0.5, "close", callbacks.on_collapse, fill=SURFACE, icon_color=TEXT_MUTED)


def _draw_overview_tab(painter: GdiPlusPainter, ui_state: ShellUiState, config: AppConfig, s: float, x: float, y: float, w: float, callbacks: ShellCallbacks) -> None:
    search_w, search_h = w - 82.0 * s, 33.0 * s
    draw_text_input(painter, ui_state.interaction, "google_search", x, y, search_w, search_h, ui_state.search_input, ui_state.search_input.focused, ui_state.search_input.placeholder, icon="search", enter_hint="↵")
    draw_pill_button(painter, ui_state.interaction, "btn_search_go", x + search_w + 8.0 * s, y, 74.0 * s, search_h, "Search", callbacks.on_submit_search, active=bool(ui_state.search_input.text), accent=SKY, radius=10.0 * s)

    row_y, row_h = y + 42.0 * s, 104.0 * s
    left_w = (w - 10.0 * s) * 0.55
    _draw_home_media_card(painter, ui_state, s, x, row_y, left_w, row_h, callbacks)
    _draw_home_discord_card(painter, ui_state, config, s, x + left_w + 10.0 * s, row_y, w - left_w - 10.0 * s, row_h, callbacks)

    strip_y, strip_h, gap = row_y + row_h + 9.0 * s, 30.0 * s, 8.0 * s
    chip_w = (w - gap * 3.0) / 4.0
    vol_pct = round(ui_state.media_audio.master_volume * 100)
    tmr = ui_state.productivity.timer
    task_pending = sum(1 for t in ui_state.productivity.tasks if not t.done)
    rack_count = len(ui_state.rack.items)

    draw_pill_button(painter, ui_state.interaction, "home_chip_vol", x, strip_y, chip_w, strip_h, "Muted" if ui_state.media_audio.master_muted else f"Vol {vol_pct}%", lambda: callbacks.on_select_tab(ExpandedTab.CONTROLS), icon="volume_mute" if ui_state.media_audio.master_muted else "volume", size_px=10.5 * s, radius=9.0 * s)
    draw_pill_button(painter, ui_state.interaction, "home_chip_tmr", x + chip_w + gap, strip_y, chip_w, strip_h, tmr.formatted_remaining() if (tmr.running or tmr.remaining_secs < tmr.total_secs) else "Timer", lambda: callbacks.on_select_tab(ExpandedTab.TIMER), active=tmr.running, accent=AMBER, icon="clock", size_px=10.5 * s, radius=9.0 * s)
    draw_pill_button(painter, ui_state.interaction, "home_chip_tasks", x + (chip_w + gap) * 2.0, strip_y, chip_w, strip_h, f"{task_pending} Tasks" if task_pending else "Tasks", lambda: callbacks.on_select_tab(ExpandedTab.TASKS), icon="check", size_px=10.5 * s, radius=9.0 * s)
    draw_pill_button(painter, ui_state.interaction, "home_chip_rack", x + (chip_w + gap) * 3.0, strip_y, chip_w, strip_h, f"{rack_count} Staged" if rack_count else "Drop Rack", lambda: callbacks.on_select_tab(ExpandedTab.RACK), active=rack_count > 0, accent=SKY, icon="folder", size_px=10.5 * s, radius=9.0 * s)

    util_y, btn_h = strip_y + strip_h + 9.0 * s, 28.0 * s
    draw_pill_button(painter, ui_state.interaction, "btn_peek", x, util_y, 88.0 * s, btn_h, "Test Peek", callbacks.on_trigger_peek, size_px=10.5 * s, radius=9.0 * s)
    if callbacks.on_toggle_capture_privacy is not None:
        draw_pill_button(painter, ui_state.interaction, "btn_privacy", x + 96.0 * s, util_y, 136.0 * s, btn_h, "Screen Privacy On" if ui_state.capture_excluded else "Screen Privacy Off", callbacks.on_toggle_capture_privacy, active=ui_state.capture_excluded, accent=VIOLET, size_px=10.5 * s, radius=9.0 * s)
        hide_x = x + 240.0 * s
    else:
        hide_x = x + 96.0 * s
    draw_pill_button(painter, ui_state.interaction, "btn_hide", hide_x, util_y, 134.0 * s, btn_h, "Hide (Ctrl+Alt+D)", callbacks.on_hide_manual, size_px=10.5 * s, radius=9.0 * s)
    draw_pill_button(painter, ui_state.interaction, "btn_collapse", x + w - 82.0 * s, util_y, 82.0 * s, btn_h, "Collapse", callbacks.on_collapse, size_px=10.5 * s, radius=9.0 * s)


def _draw_home_media_card(painter: GdiPlusPainter, ui_state: ShellUiState, s: float, x: float, y: float, w: float, h: float, callbacks: ShellCallbacks) -> None:
    media = ui_state.media_audio.media
    dom = painter.get_image_dominant_color(media.thumbnail_bytes) if media.has_session else 0
    draw_card(painter, x, y, w, h, radius=12.0 * s, accent=dom or (SKY if media.has_session else 0), elevated=True)
    art_sz, ax, ay = 44.0 * s, x + 12.0 * s, y + 12.0 * s
    ui_state.interaction.register_button("home_open_media", Rect(ax, ay, w - 24.0 * s, art_sz), lambda: callbacks.on_select_tab(ExpandedTab.MEDIA))
    if media.has_session and media.thumbnail_bytes:
        painter.draw_image_bytes(media.thumbnail_bytes, ax, ay, art_sz, art_sz, radius=9.0 * s)
    else:
        painter.fill_round_rect(ax, ay, art_sz, art_sz, 9.0 * s, argb(28, 255, 255, 255))
        draw_icon(painter, "play" if media.is_playing else "spark", ax + art_sz * 0.5, ay + art_sz * 0.5, 16.0 * s, TEXT_MUTED)
    tx, tw = ax + art_sz + 10.0 * s, w - (ax + art_sz + 10.0 * s - x) - 12.0 * s
    if media.has_session and media.title:
        painter.draw_text(media.title, tx, ay + 2.0 * s, tw, 18.0 * s, TEXT_PRIMARY, size_px=12.0 * s, bold=True)
        painter.draw_text(media.artist or "Playing", tx, ay + 22.0 * s, tw, 16.0 * s, TEXT_MUTED, size_px=10.5 * s)
    else:
        painter.draw_text("No Active Media", tx, ay + 4.0 * s, tw, 18.0 * s, TEXT_SECONDARY, size_px=11.5 * s, bold=True)
        painter.draw_text("Media keys ready", tx, ay + 22.0 * s, tw, 16.0 * s, TEXT_DIM, size_px=10.0 * s)
    ctrl_cy, cx = y + h - 22.0 * s, x + w * 0.5
    draw_icon_button(painter, ui_state.interaction, "home_prev", cx - 42.0 * s, ctrl_cy, 13.0 * s, "prev", callbacks.media_audio.on_prev_track)
    play_accent = dom or EMERALD
    draw_icon_button(painter, ui_state.interaction, "home_play", cx, ctrl_cy, 15.0 * s, "pause" if media.is_playing else "play", callbacks.media_audio.on_toggle_play, fill=blend_alpha(play_accent, 78 if media.has_session else 32), icon_color=TEXT_PRIMARY, accent=play_accent, active=media.is_playing)
    draw_icon_button(painter, ui_state.interaction, "home_next", cx + 42.0 * s, ctrl_cy, 13.0 * s, "next", callbacks.media_audio.on_next_track)


def _draw_home_discord_card(painter: GdiPlusPainter, ui_state: ShellUiState, config: AppConfig, s: float, x: float, y: float, w: float, h: float, callbacks: ShellCallbacks) -> None:
    dc = ui_state.discord
    dot_col = ROSE if dc.muted else (EMERALD if dc.in_call else (SKY if dc.discord_running else TEXT_DIM))
    draw_card(painter, x, y, w, h, radius=12.0 * s, accent=dot_col if dc.in_call else 0, elevated=True)
    painter.fill_circle(x + 15.0 * s, y + 20.0 * s, 4.0 * s, dot_col)
    status = f"In Voice{' · Muted' if dc.muted else ''}" if dc.in_call else ("Discord Ready" if dc.discord_running else "Discord Idle")
    painter.draw_text(status, x + 24.0 * s, y + 11.0 * s, w - 34.0 * s, 18.0 * s, TEXT_PRIMARY if dc.discord_running else TEXT_SECONDARY, size_px=11.5 * s, bold=True)
    sub = f"via {dc.mode_label}" + (f" ({dc.channel_name})" if dc.channel_name else "")
    painter.draw_text(sub, x + 12.0 * s, y + 31.0 * s, w - 24.0 * s, 15.0 * s, TEXT_MUTED, size_px=10.0 * s)
    btn_y, btn_h, bw = y + h - 38.0 * s, 26.0 * s, (w - 30.0 * s) / 3.0
    draw_pill_button(painter, ui_state.interaction, "dc_mute", x + 10.0 * s, btn_y, bw, btn_h, "Unmute" if dc.muted else "Mute", callbacks.on_discord_mute, active=dc.muted, accent=ROSE, size_px=9.8 * s, radius=8.0 * s)
    draw_pill_button(painter, ui_state.interaction, "dc_deaf", x + 15.0 * s + bw, btn_y, bw, btn_h, "Undeaf" if dc.deafened else "Deafen", callbacks.on_discord_deafen, active=dc.deafened, accent=AMBER, size_px=9.8 * s, radius=8.0 * s)
    if dc.in_call:
        draw_pill_button(painter, ui_state.interaction, "dc_end", x + 20.0 * s + bw * 2.0, btn_y, bw, btn_h, "Leave", callbacks.on_discord_disconnect, active=True, accent=ROSE, size_px=9.8 * s, radius=8.0 * s)
    else:
        draw_pill_button(painter, ui_state.interaction, "dc_hotkey", x + 20.0 * s + bw * 2.0, btn_y, bw, btn_h, "Keys" if config.discord_mute_hotkey else "+Keys", callbacks.on_discord_toggle_hotkey_preset, active=bool(config.discord_mute_hotkey), accent=SKY, size_px=9.8 * s, radius=8.0 * s)
