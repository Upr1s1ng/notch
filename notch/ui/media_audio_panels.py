from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable

from notch.modules.audio import AudioSessionInfo
from notch.modules.brightness import BrightnessState
from notch.modules.lyrics_lrclib import LyricsResult, active_line_index
from notch.modules.media_smtc import MediaSnapshot
from notch.ui.gdiplus import GdiPlusPainter
from notch.ui.theme import (
    AMBER, EMERALD, ROSE, SKY, SURFACE, TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
    blend_alpha, draw_card, draw_icon, draw_icon_button, draw_pill_button, draw_slider,
)
from notch.ui.widgets import UiInteractionState


@dataclass
class MediaAudioState:
    media: MediaSnapshot = field(default_factory=MediaSnapshot)
    lyrics: LyricsResult | None = None
    master_volume: float = 0.5
    master_muted: bool = False
    brightness: BrightnessState = field(default_factory=BrightnessState)
    sessions: list[AudioSessionInfo] = field(default_factory=list)
    last_track_key: str = ""
    session_scroll: int = 0


@dataclass
class MediaAudioCallbacks:
    on_toggle_play: Callable[[], None]
    on_next_track: Callable[[], None]
    on_prev_track: Callable[[], None]
    on_seek: Callable[[float], None]
    on_cycle_source: Callable[[], None]
    on_set_master_volume: Callable[[float], None]
    on_toggle_master_mute: Callable[[], None]
    on_set_brightness: Callable[[float], None]
    on_set_app_volume: Callable[[int, float], None]
    on_toggle_app_mute: Callable[[int, bool], None]


def draw_media_panel(
    painter: GdiPlusPainter, interaction: UiInteractionState, state: MediaAudioState,
    s: float, x: float, y: float, w: float, cb: MediaAudioCallbacks,
) -> None:
    media = state.media
    dom = painter.get_image_dominant_color(media.thumbnail_bytes) if media.has_session else 0
    accent = dom or SKY
    hero_h = 114.0 * s
    draw_card(painter, x, y, w, hero_h, radius=12.0 * s, accent=accent if media.has_session else 0, elevated=True)

    art_sz = 68.0 * s
    ax, ay = x + 12.0 * s, y + 12.0 * s
    if media.has_session and media.thumbnail_bytes:
        painter.fill_radial_glow(ax + art_sz * 0.5, ay + art_sz * 0.5, art_sz * 0.72, blend_alpha(accent, 95))
        painter.draw_image_bytes(media.thumbnail_bytes, ax, ay, art_sz, art_sz, radius=11.0 * s)
    else:
        painter.fill_round_rect(ax, ay, art_sz, art_sz, 11.0 * s, SURFACE)
        draw_icon(painter, "play", ax + art_sz * 0.5, ay + art_sz * 0.5, 22.0 * s, TEXT_MUTED)

    meta_x = ax + art_sz + 14.0 * s
    src_w = 88.0 * s
    meta_w = w - (meta_x - x) - src_w - 18.0 * s
    if media.has_session:
        painter.draw_text(media.title or "Unknown Track", meta_x, ay + 2.0 * s, meta_w, 20.0 * s, TEXT_PRIMARY, size_px=13.5 * s, bold=True)
        sub = " · ".join(p for p in (media.artist, media.album) if p) or "Active Media"
        painter.draw_text(sub, meta_x, ay + 24.0 * s, meta_w, 16.0 * s, TEXT_SECONDARY, size_px=11.0 * s)
    else:
        painter.draw_text("No Active Playback", meta_x, ay + 8.0 * s, meta_w, 20.0 * s, TEXT_SECONDARY, size_px=13.0 * s, bold=True)
        painter.draw_text("Play Spotify, Chrome, or media apps", meta_x, ay + 30.0 * s, meta_w, 16.0 * s, TEXT_DIM, size_px=10.5 * s)

    src_label = (media.source_app_id or "Source").split("!")[-1].replace(".exe", "")[:11]
    draw_pill_button(
        painter, interaction, "media_source", x + w - src_w - 10.0 * s, ay + 2.0 * s, src_w, 22.0 * s,
        src_label, cb.on_cycle_source, active=len(media.available_sources) > 1, accent=accent, size_px=9.8 * s, radius=8.0 * s,
    )

    ctrl_cy = ay + 56.0 * s
    draw_icon_button(painter, interaction, "media_prev", meta_x + 14.0 * s, ctrl_cy, 13.0 * s, "prev", cb.on_prev_track)
    draw_icon_button(
        painter, interaction, "media_play", meta_x + 50.0 * s, ctrl_cy, 15.5 * s,
        "pause" if media.is_playing else "play", cb.on_toggle_play,
        fill=blend_alpha(accent, 92 if media.has_session else 34), icon_color=TEXT_PRIMARY, accent=accent, active=media.is_playing,
    )
    draw_icon_button(painter, interaction, "media_next", meta_x + 86.0 * s, ctrl_cy, 13.0 * s, "next", cb.on_next_track)

    pos = media.live_position_secs() if media.has_session else 0.0
    dur = max(1.0, media.duration_secs) if media.has_session else 1.0
    ratio = max(0.0, min(1.0, pos / dur)) if media.has_session else 0.0
    bar_y = y + hero_h - 24.0 * s
    painter.draw_text(_fmt_time(pos), x + 12.0 * s, bar_y, 38.0 * s, 16.0 * s, TEXT_MUTED, size_px=10.0 * s, mono=True)
    painter.draw_text(_fmt_time(media.duration_secs if media.has_session else 0.0), x + w - 50.0 * s, bar_y, 38.0 * s, 16.0 * s, TEXT_MUTED, size_px=10.0 * s, mono=True, align=2)
    draw_slider(painter, interaction, "media_seek", x + 54.0 * s, bar_y + 1.0 * s, w - 108.0 * s, 14.0 * s, ratio, accent, lambda r: cb.on_seek(r * dur))
    _draw_lyrics_box(painter, state.lyrics, pos, accent, s, x, y + hero_h + 9.0 * s, w, 108.0 * s)


def _draw_lyrics_box(painter: GdiPlusPainter, lyrics: LyricsResult | None, pos_secs: float, accent: int, s: float, x: float, y: float, w: float, h: float) -> None:
    draw_card(painter, x, y, w, h, radius=12.0 * s)
    painter.draw_text("LYRICS", x + 12.0 * s, y + 7.0 * s, 80.0 * s, 14.0 * s, TEXT_DIM, size_px=9.0 * s, bold=True)
    if lyrics is None or (not lyrics.synced and not lyrics.plain):
        msg = "Fetching synced lyrics from LRCLIB..." if (lyrics and lyrics.loading) else "Play a track to load live synced lyrics"
        painter.draw_text(msg, x + 14.0 * s, y + 36.0 * s, w - 28.0 * s, 26.0 * s, TEXT_MUTED, size_px=11.0 * s, align=1)
        return
    body_y = y + 22.0 * s
    if lyrics.synced:
        idx = active_line_index(lyrics.synced, pos_secs)
        for slot, line_idx in enumerate((idx - 1, idx, idx + 1)):
            if 0 <= line_idx < len(lyrics.synced):
                ly = body_y + slot * 26.0 * s
                is_cur = slot == 1
                if is_cur:
                    painter.fill_round_rect(x + 10.0 * s, ly, w - 20.0 * s, 24.0 * s, 7.0 * s, blend_alpha(accent, 34))
                painter.draw_text(lyrics.synced[line_idx].text, x + 16.0 * s, ly, w - 32.0 * s, 24.0 * s, TEXT_PRIMARY if is_cur else TEXT_DIM, size_px=(12.0 if is_cur else 10.5) * s, bold=is_cur, align=1)
    else:
        for slot, line in enumerate(lyrics.plain[:3]):
            painter.draw_text(line, x + 14.0 * s, body_y + slot * 24.0 * s, w - 28.0 * s, 22.0 * s, TEXT_SECONDARY if slot == 0 else TEXT_MUTED, size_px=11.0 * s, align=1)


def draw_audio_panel(
    painter: GdiPlusPainter, interaction: UiInteractionState, state: MediaAudioState,
    s: float, x: float, y: float, w: float, cb: MediaAudioCallbacks,
) -> None:
    card_w = (w - 10.0 * s) * 0.5
    card_h = 74.0 * s
    draw_card(painter, x, y, card_w, card_h, radius=12.0 * s, accent=ROSE if state.master_muted else EMERALD, elevated=True)
    vol_pct = round(state.master_volume * 100)
    draw_icon_button(
        painter, interaction, "aud_master_mute", x + 22.0 * s, y + 22.0 * s, 12.0 * s,
        "volume_mute" if state.master_muted else "volume", cb.on_toggle_master_mute,
        fill=blend_alpha(ROSE if state.master_muted else EMERALD, 45), icon_color=ROSE if state.master_muted else EMERALD,
    )
    painter.draw_text("Master Volume", x + 40.0 * s, y + 12.0 * s, card_w - 86.0 * s, 20.0 * s, TEXT_PRIMARY, size_px=11.5 * s, bold=True)
    painter.draw_text("Muted" if state.master_muted else f"{vol_pct}%", x + card_w - 50.0 * s, y + 12.0 * s, 38.0 * s, 20.0 * s, ROSE if state.master_muted else EMERALD, size_px=11.0 * s, bold=True, mono=True, align=2)
    draw_slider(painter, interaction, "aud_master_slider", x + 14.0 * s, y + 45.0 * s, card_w - 28.0 * s, 16.0 * s, 0.0 if state.master_muted else state.master_volume, ROSE if state.master_muted else EMERALD, cb.on_set_master_volume)

    bx = x + card_w + 10.0 * s
    br = state.brightness
    draw_card(painter, bx, y, card_w, card_h, radius=12.0 * s, accent=AMBER if br.supported else 0, elevated=True)
    draw_icon(painter, "sun", bx + 22.0 * s, y + 22.0 * s, 16.0 * s, AMBER if br.supported else TEXT_DIM)
    painter.draw_text("Brightness", bx + 38.0 * s, y + 12.0 * s, card_w - 86.0 * s, 20.0 * s, TEXT_PRIMARY, size_px=11.5 * s, bold=True)
    painter.draw_text(f"{round(br.level * 100)}%" if br.supported else "N/A", bx + card_w - 50.0 * s, y + 12.0 * s, 38.0 * s, 20.0 * s, AMBER if br.supported else TEXT_DIM, size_px=11.0 * s, bold=True, mono=True, align=2)
    if br.supported:
        draw_slider(painter, interaction, "aud_brightness_slider", bx + 14.0 * s, y + 45.0 * s, card_w - 28.0 * s, 16.0 * s, br.level, AMBER, cb.on_set_brightness)
    else:
        painter.draw_text(f"Unavailable ({br.backend})", bx + 14.0 * s, y + 44.0 * s, card_w - 28.0 * s, 18.0 * s, TEXT_DIM, size_px=10.0 * s)

    mix_y = y + card_h + 10.0 * s
    mix_h = 146.0 * s
    draw_card(painter, x, mix_y, w, mix_h, radius=12.0 * s)
    painter.draw_text("APP MIXER", x + 12.0 * s, mix_y + 9.0 * s, 120.0 * s, 16.0 * s, TEXT_DIM, size_px=9.5 * s, bold=True)
    sessions = state.sessions
    max_rows = 3
    if len(sessions) > max_rows:
        draw_pill_button(painter, interaction, "aud_scroll_up", x + w - 58.0 * s, mix_y + 7.0 * s, 22.0 * s, 20.0 * s, "↑", lambda: setattr(state, "session_scroll", max(0, state.session_scroll - 1)), size_px=10.0 * s, radius=6.0 * s)
        draw_pill_button(painter, interaction, "aud_scroll_dn", x + w - 32.0 * s, mix_y + 7.0 * s, 22.0 * s, 20.0 * s, "↓", lambda: setattr(state, "session_scroll", min(len(sessions) - max_rows, state.session_scroll + 1)), size_px=10.0 * s, radius=6.0 * s)

    if not sessions:
        painter.draw_text("No active app audio streams", x + 14.0 * s, mix_y + 62.0 * s, w - 28.0 * s, 22.0 * s, TEXT_MUTED, size_px=11.0 * s, align=1)
        return
    for idx, sess in enumerate(sessions[state.session_scroll : state.session_scroll + max_rows]):
        ry = mix_y + 32.0 * s + idx * 35.0 * s
        painter.draw_text(sess.name, x + 12.0 * s, ry + 4.0 * s, 124.0 * s, 22.0 * s, TEXT_PRIMARY if not sess.muted else TEXT_MUTED, size_px=11.0 * s, bold=True)
        draw_icon_button(painter, interaction, f"app_mute_{sess.pid}", x + 148.0 * s, ry + 15.0 * s, 11.0 * s, "volume_mute" if sess.muted else "volume", lambda p=sess.pid, m=sess.muted: cb.on_toggle_app_mute(p, not m), fill=blend_alpha(ROSE, 48) if sess.muted else SURFACE, icon_color=ROSE if sess.muted else TEXT_SECONDARY)
        draw_slider(painter, interaction, f"app_vol_{sess.pid}", x + 170.0 * s, ry + 7.0 * s, w - 224.0 * s, 16.0 * s, 0.0 if sess.muted else sess.volume, SKY, lambda r, p=sess.pid: cb.on_set_app_volume(p, r))
        painter.draw_text(f"{round(sess.volume * 100)}%", x + w - 48.0 * s, ry + 5.0 * s, 36.0 * s, 20.0 * s, TEXT_MUTED, size_px=10.0 * s, mono=True, align=2)


def _fmt_time(seconds: float) -> str:
    sec = max(0, int(seconds))
    return f"{sec // 60}:{sec % 60:02d}"
