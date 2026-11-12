use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::config::{AppConfig, NotchStyle};
use crate::modules::audio::AppAudioSession;
use crate::modules::calendar::AgendaItem;
use crate::modules::clipboard::ClipEntry;
use crate::modules::discord::DiscordVoiceState;
use crate::modules::lyrics::LyricsResult;
use crate::modules::media::MediaSnapshot;
use crate::modules::shelf::ShelfItem;
use crate::modules::shortcuts::AppShortcut;
use crate::modules::tasks::{TaskFilter, TaskItem};
use crate::modules::timer::TimerState;
use crate::os::d2d::D2dRenderer;
use crate::state::{ExpandedTab, MascotMood, NotchState, NotchStateMachine};
use crate::ui::theme::{
    AMBER, BG_BOTTOM, BG_MINIMAL, BG_TOP, BORDER_SPECULAR, EMERALD, ROSE, SKY, SURFACE,
    SURFACE_HOVER, TAB_ACTIVE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UiAction {
    ToggleExpand,
    Collapse,
    TogglePin,
    SelectTab(ExpandedTab),
    CycleStyle,
    ToggleMascot,
    TogglePrivacy,
    TriggerPeekDemo,
    FocusInput(u8), // 0 = google_search, 1 = task_input, 2 = app_input
    SubmitSearch,
    DiscordMute,
    DiscordDeafen,
    DiscordDisconnect,
    MediaTogglePlay,
    MediaNext,
    MediaPrev,
    MediaCycleSource,
    MediaToggleLyrics,
    SeekLyric(u32), // timestamp in milliseconds
    SeekRatio(f32),
    SetMasterVol(f32),
    ToggleMasterMute,
    SetBrightness(f32),
    SetAppVol(u32, f32),
    SetTaskFilter(TaskFilter),
    AddTask,
    ToggleTask(u64),
    DeleteTask(u64),
    TimerToggle,
    TimerReset,
    TimerPreset(u64),
    LaunchShortcut(usize),
    RemoveShortcut(u64),
    AddShortcut,
    OpenShelfItem(usize),
    RemoveShelfItem(usize),
    ClearShelf,
    CopyClip(usize),
}

#[derive(Default)]
pub struct UiHitMap {
    pub buttons: Vec<(Rect, UiAction)>,
    pub sliders: Vec<(Rect, u8, u32)>, // 0=seek, 1=master_vol, 2=brightness, 3=app_vol(pid)
    pub hovered_idx: Option<usize>,
}

impl UiHitMap {
    pub fn clear(&mut self) {
        self.buttons.clear();
        self.sliders.clear();
    }

    pub fn push(&mut self, rect: Rect, action: UiAction) {
        self.buttons.push((rect, action));
    }

    pub fn push_slider(&mut self, rect: Rect, kind: u8, id: u32) {
        self.sliders.push((rect, kind, id));
    }

    pub fn hit_test(&self, px: f32, py: f32) -> Option<UiAction> {
        for (r, kind, id) in self.sliders.iter().rev() {
            if r.contains(px, py) {
                let ratio = ((px - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
                return Some(match *kind {
                    0 => UiAction::SeekRatio(ratio),
                    1 => UiAction::SetMasterVol(ratio),
                    2 => UiAction::SetBrightness(ratio),
                    _ => UiAction::SetAppVol(*id, ratio),
                });
            }
        }
        self.buttons
            .iter()
            .rev()
            .find(|(r, _)| r.contains(px, py))
            .map(|(_, a)| *a)
    }

    pub fn update_hover(&mut self, px: f32, py: f32) -> bool {
        let next = self.buttons.iter().rposition(|(r, _)| r.contains(px, py));
        let changed = next != self.hovered_idx;
        self.hovered_idx = next;
        changed
    }
}

pub struct ShellData<'a> {
    pub media: &'a MediaSnapshot,
    pub lyrics: &'a LyricsResult,
    pub show_lyrics: bool,
    pub master_vol: f32,
    pub master_muted: bool,
    pub brightness: f32,
    pub app_sessions: &'a [AppAudioSession],
    pub discord: &'a DiscordVoiceState,
    pub search_text: &'a str,
    pub task_input: &'a str,
    pub app_input: &'a str,
    pub focused_input: Option<u8>,
    pub task_filter: TaskFilter,
    pub tasks: &'a [&'a TaskItem],
    pub agenda: &'a [AgendaItem],
    pub timer: &'a TimerState,
    pub shortcuts: &'a [AppShortcut],
    pub shelf: &'a [ShelfItem],
    pub clips: &'a [ClipEntry],
}

fn local_clock_hhmm() -> String {
    unsafe {
        let st = GetLocalTime();
        format!("{:02}:{:02}", st.wHour, st.wMinute)
    }
}

fn battery_percent() -> Option<(u8, bool)> {
    unsafe {
        let mut sps = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut sps).is_ok() && sps.BatteryLifePercent <= 100 {
            return Some((sps.BatteryLifePercent, sps.ACLineStatus == 1));
        }
        None
    }
}

pub fn draw_shell(
    d2d: &D2dRenderer,
    sm: &NotchStateMachine,
    cfg: &AppConfig,
    data: &ShellData<'_>,
    canvas_w: f32,
    hits: &mut UiHitMap,
) {
    hits.clear();
    if sm.state == NotchState::Hidden {
        return;
    }

    let w = sm.width.value;
    let h = sm.height.value;
    let x = (canvas_w - w) * 0.5;
    let y = sm.top_offset.value;
    let r = sm.corner_radius.value;

    match cfg.style {
        NotchStyle::TopEdge => {
            d2d.draw_top_edge_notch(x, y, w, h, r, 10.0, BG_TOP, BG_BOTTOM, BORDER_SPECULAR);
        }
        NotchStyle::FloatingPill => {
            if let Ok(grad) = d2d.linear_gradient(y, y + h, BG_TOP, BG_BOTTOM) {
                let rr = windows::Win32::Graphics::Direct2D::D2D1_ROUNDED_RECT {
                    rect: windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    },
                    radiusX: r,
                    radiusY: r,
                };
                unsafe {
                    d2d.target.FillRoundedRectangle(&rr, &grad);
                }
            }
            d2d.draw_round_rect(x, y, w, h, r, BORDER_SPECULAR, 1.0);
        }
        NotchStyle::Minimal if sm.state == NotchState::Collapsed => {
            d2d.fill_round_rect(x, y, w, h, 5.0, BG_MINIMAL);
            let dot_col = if data.media.is_playing { EMERALD } else { SKY };
            d2d.fill_circle(x + w * 0.5, y + h * 0.5, 2.2, dot_col);
            hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
            return;
        }
        NotchStyle::Minimal => {
            d2d.draw_top_edge_notch(x, y, w, h, r, 10.0, BG_TOP, BG_BOTTOM, BORDER_SPECULAR);
        }
    }

    match sm.state {
        NotchState::Collapsed => {
            hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
            let has_live = data.media.is_playing || data.timer.running || data.discord.in_call;
            if cfg.mascot_enabled && !has_live && sm.mascot_mood != MascotMood::Clock {
                draw_mascot_eyes(d2d, x, y, w, h, sm.mascot_mood);
            } else {
                let clock = local_clock_hhmm();
                d2d.draw_text(
                    &clock,
                    x + 14.0,
                    y,
                    52.0,
                    h,
                    TEXT_PRIMARY,
                    &d2d.font_mono,
                    0,
                );
                if data.media.is_playing {
                    d2d.draw_text(
                        &data.media.title,
                        x + 68.0,
                        y,
                        w - 116.0,
                        h,
                        TEXT_SECONDARY,
                        &d2d.font_small,
                        0,
                    );
                } else if data.timer.running {
                    d2d.draw_text(
                        &data.timer.formatted_remaining(),
                        x + 68.0,
                        y,
                        w - 116.0,
                        h,
                        AMBER,
                        &d2d.font_mono,
                        0,
                    );
                }
                if data.discord.discord_running {
                    let dc = if data.discord.muted { ROSE } else { EMERALD };
                    d2d.fill_circle(x + w - 40.0, y + h * 0.5, 3.5, dc);
                }
                let ring_col = match battery_percent() {
                    Some((pct, true)) if pct < 100 => EMERALD,
                    Some((pct, _)) if pct <= 20 => AMBER,
                    _ => SKY,
                };
                d2d.draw_circle(x + w - 22.0, y + h * 0.5, 7.0, ring_col, 2.0);
            }
        }
        NotchState::Peek => {
            hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
            if let Some(p) = &sm.peek {
                d2d.fill_circle(x + 24.0, y + h * 0.5, 11.0, TAB_ACTIVE);
                d2d.fill_circle(x + 24.0, y + h * 0.5, 4.0, SKY);
                d2d.draw_text(
                    &p.title,
                    x + 46.0,
                    y + 10.0,
                    w - 64.0,
                    22.0,
                    TEXT_PRIMARY,
                    &d2d.font_bold,
                    0,
                );
                d2d.draw_text(
                    &p.subtitle,
                    x + 46.0,
                    y + 32.0,
                    w - 64.0,
                    20.0,
                    TEXT_SECONDARY,
                    &d2d.font_small,
                    0,
                );
            }
        }
        NotchState::Expanded => {
            draw_expanded(d2d, sm, cfg, data, x, y, w, h, hits);
        }
        NotchState::Hidden => {}
    }
}

fn draw_mascot_eyes(d2d: &D2dRenderer, x: f32, y: f32, w: f32, h: f32, mood: MascotMood) {
    let cx = x + w * 0.5;
    let cy = y + h * 0.5;
    match mood {
        MascotMood::Doze => {
            d2d.draw_line(cx - 16.0, cy, cx - 6.0, cy + 2.0, TEXT_SECONDARY, 2.0);
            d2d.draw_line(cx + 6.0, cy + 2.0, cx + 16.0, cy, TEXT_SECONDARY, 2.0);
            d2d.draw_text(
                "zZ",
                cx + 24.0,
                y + 4.0,
                24.0,
                h - 8.0,
                TEXT_MUTED,
                &d2d.font_small,
                0,
            );
        }
        MascotMood::Blink => {
            d2d.draw_line(cx - 15.0, cy, cx - 7.0, cy, TEXT_PRIMARY, 2.2);
            d2d.draw_line(cx + 7.0, cy, cx + 15.0, cy, TEXT_PRIMARY, 2.2);
        }
        MascotMood::Wave => {
            d2d.fill_circle(cx - 11.0, cy, 4.2, SKY);
            d2d.fill_circle(cx + 11.0, cy, 4.2, SKY);
            d2d.draw_text(
                "hi",
                cx + 22.0,
                y + 4.0,
                24.0,
                h - 8.0,
                EMERALD,
                &d2d.font_small,
                0,
            );
        }
        MascotMood::Awake | MascotMood::Clock => {
            d2d.fill_circle(cx - 11.0, cy, 4.0, TEXT_PRIMARY);
            d2d.fill_circle(cx + 11.0, cy, 4.0, TEXT_PRIMARY);
        }
    }
}

fn draw_input(
    d2d: &D2dRenderer,
    r: Rect,
    text: &str,
    placeholder: &str,
    focused: bool,
    id: u8,
    hits: &mut UiHitMap,
) {
    d2d.fill_round_rect(r.x, r.y, r.w, r.h, 8.0, SURFACE);
    d2d.draw_round_rect(
        r.x,
        r.y,
        r.w,
        r.h,
        8.0,
        if focused { SKY } else { BORDER_SPECULAR },
        1.0,
    );
    let shown = if text.is_empty() { placeholder } else { text };
    let col = if text.is_empty() {
        TEXT_MUTED
    } else {
        TEXT_PRIMARY
    };
    d2d.draw_text(
        shown,
        r.x + 10.0,
        r.y,
        r.w - 20.0,
        r.h,
        col,
        &d2d.font_small,
        0,
    );
    hits.push(r, UiAction::FocusInput(id));
}

fn draw_slider(
    d2d: &D2dRenderer,
    r: Rect,
    ratio: f32,
    col: windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F,
    kind: u8,
    id: u32,
    hits: &mut UiHitMap,
) {
    let mid_y = r.y + r.h * 0.5 - 3.0;
    d2d.fill_round_rect(r.x, mid_y, r.w, 6.0, 3.0, SURFACE_HOVER);
    d2d.fill_round_rect(
        r.x,
        mid_y,
        (r.w * ratio.clamp(0.0, 1.0)).max(6.0),
        6.0,
        3.0,
        col,
    );
    hits.push_slider(r, kind, id);
}

fn draw_expanded(
    d2d: &D2dRenderer,
    sm: &NotchStateMachine,
    cfg: &AppConfig,
    data: &ShellData<'_>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hits: &mut UiHitMap,
) {
    let pad = 14.0;
    let top_y = y + 10.0;
    let tab_w = 52.0;
    let mut tx = x + pad;

    for tab in ExpandedTab::ALL {
        let r = Rect {
            x: tx,
            y: top_y,
            w: tab_w,
            h: 26.0,
        };
        d2d.fill_round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            9.0,
            if sm.active_tab == tab {
                TAB_ACTIVE
            } else {
                SURFACE
            },
        );
        d2d.draw_text(
            tab.label(),
            r.x,
            r.y,
            r.w,
            r.h,
            if sm.active_tab == tab {
                TEXT_PRIMARY
            } else {
                TEXT_SECONDARY
            },
            &d2d.font_small,
            1,
        );
        hits.push(r, UiAction::SelectTab(tab));
        tx += tab_w + 4.0;
    }

    let pin_r = Rect {
        x: x + w - 68.0,
        y: top_y,
        w: 26.0,
        h: 26.0,
    };
    d2d.fill_round_rect(
        pin_r.x,
        pin_r.y,
        pin_r.w,
        pin_r.h,
        8.0,
        if sm.pinned_open { TAB_ACTIVE } else { SURFACE },
    );
    d2d.draw_text(
        "P",
        pin_r.x,
        pin_r.y,
        pin_r.w,
        pin_r.h,
        TEXT_PRIMARY,
        &d2d.font_small,
        1,
    );
    hits.push(pin_r, UiAction::TogglePin);

    let close_r = Rect {
        x: x + w - 38.0,
        y: top_y,
        w: 26.0,
        h: 26.0,
    };
    d2d.fill_round_rect(
        close_r.x,
        close_r.y,
        close_r.w,
        close_r.h,
        8.0,
        SURFACE_HOVER,
    );
    d2d.draw_text(
        "-",
        close_r.x,
        close_r.y,
        close_r.w,
        close_r.h,
        TEXT_PRIMARY,
        &d2d.font_bold,
        1,
    );
    hits.push(close_r, UiAction::Collapse);

    let bx = x + pad;
    let by = top_y + 34.0;
    let bw = w - pad * 2.0;
    let bh = h - (by - y) - pad;

    match sm.active_tab {
        ExpandedTab::Home => {
            // Google Search Bar across top
            draw_input(
                d2d,
                Rect {
                    x: bx,
                    y: by,
                    w: bw - 68.0,
                    h: 30.0,
                },
                data.search_text,
                "Search Google... (Ctrl+Alt+Space)",
                data.focused_input == Some(0),
                0,
                hits,
            );
            let go_r = Rect {
                x: bx + bw - 62.0,
                y: by,
                w: 62.0,
                h: 30.0,
            };
            d2d.fill_round_rect(go_r.x, go_r.y, go_r.w, go_r.h, 8.0, TAB_ACTIVE);
            d2d.draw_text(
                "Search",
                go_r.x,
                go_r.y,
                go_r.w,
                go_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(go_r, UiAction::SubmitSearch);

            let cy = by + 38.0;
            let ch = bh - 38.0;
            let col_w = (bw - 10.0) * 0.5;
            d2d.fill_round_rect(bx, cy, col_w, ch, 12.0, SURFACE);
            d2d.draw_text(
                &local_clock_hhmm(),
                bx + 12.0,
                cy + 8.0,
                120.0,
                26.0,
                TEXT_PRIMARY,
                &d2d.font_bold,
                0,
            );
            d2d.draw_text(
                "M  T  W  T  F  S  S",
                bx + 12.0,
                cy + 36.0,
                col_w - 24.0,
                20.0,
                SKY,
                &d2d.font_small,
                0,
            );
            let next_task = data
                .tasks
                .first()
                .map(|t| t.title.as_str())
                .unwrap_or("No pending tasks");
            d2d.draw_text(
                next_task,
                bx + 12.0,
                cy + 62.0,
                col_w - 24.0,
                20.0,
                TEXT_SECONDARY,
                &d2d.font_small,
                0,
            );

            // Discord Voice Controls
            let dc_y = cy + ch - 44.0;
            d2d.draw_text(
                if data.discord.discord_running {
                    "Discord Voice"
                } else {
                    "Discord (Idle)"
                },
                bx + 12.0,
                dc_y - 20.0,
                col_w - 24.0,
                18.0,
                TEXT_MUTED,
                &d2d.font_small,
                0,
            );
            let m_r = Rect {
                x: bx + 12.0,
                y: dc_y,
                w: 72.0,
                h: 30.0,
            };
            let d_r = Rect {
                x: bx + 90.0,
                y: dc_y,
                w: 72.0,
                h: 30.0,
            };
            let x_r = Rect {
                x: bx + 168.0,
                y: dc_y,
                w: 72.0,
                h: 30.0,
            };
            d2d.fill_round_rect(
                m_r.x,
                m_r.y,
                m_r.w,
                m_r.h,
                8.0,
                if data.discord.muted {
                    ROSE
                } else {
                    SURFACE_HOVER
                },
            );
            d2d.draw_text(
                "Mute",
                m_r.x,
                m_r.y,
                m_r.w,
                m_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(m_r, UiAction::DiscordMute);
            d2d.fill_round_rect(
                d_r.x,
                d_r.y,
                d_r.w,
                d_r.h,
                8.0,
                if data.discord.deafened {
                    ROSE
                } else {
                    SURFACE_HOVER
                },
            );
            d2d.draw_text(
                "Deafen",
                d_r.x,
                d_r.y,
                d_r.w,
                d_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(d_r, UiAction::DiscordDeafen);
            d2d.fill_round_rect(x_r.x, x_r.y, x_r.w, x_r.h, 8.0, SURFACE_HOVER);
            d2d.draw_text(
                "Leave",
                x_r.x,
                x_r.y,
                x_r.w,
                x_r.h,
                TEXT_SECONDARY,
                &d2d.font_small,
                1,
            );
            hits.push(x_r, UiAction::DiscordDisconnect);

            let rx = bx + col_w + 10.0;
            d2d.fill_round_rect(rx, cy, col_w, ch, 12.0, SURFACE);
            let btns = [
                (format!("Style: {:?}", cfg.style), UiAction::CycleStyle),
                (
                    format!("Mascot: {}", if cfg.mascot_enabled { "On" } else { "Off" }),
                    UiAction::ToggleMascot,
                ),
                (
                    format!(
                        "Privacy: {}",
                        if cfg.exclude_from_capture {
                            "Hidden"
                        } else {
                            "Visible"
                        }
                    ),
                    UiAction::TogglePrivacy,
                ),
                ("Test 2.5s Peek Alert".into(), UiAction::TriggerPeekDemo),
            ];
            for (i, (label, act)) in btns.into_iter().enumerate() {
                let br = Rect {
                    x: rx + 10.0,
                    y: cy + 10.0 + (i as f32) * 42.0,
                    w: col_w - 20.0,
                    h: 34.0,
                };
                d2d.fill_round_rect(br.x, br.y, br.w, br.h, 8.0, SURFACE_HOVER);
                d2d.draw_text(
                    &label,
                    br.x + 10.0,
                    br.y,
                    br.w - 20.0,
                    br.h,
                    TEXT_PRIMARY,
                    &d2d.font_small,
                    0,
                );
                hits.push(br, act);
            }
        }
        ExpandedTab::Media => {
            d2d.fill_round_rect(bx, by, bw, bh, 12.0, SURFACE);
            let title = if data.media.has_session {
                &data.media.title
            } else {
                "No Active Media"
            };
            let artist = if data.media.has_session {
                &data.media.artist
            } else {
                "Play Spotify, Apple Music, or YouTube"
            };
            d2d.draw_text(
                title,
                bx + 14.0,
                by + 10.0,
                bw - 140.0,
                24.0,
                TEXT_PRIMARY,
                &d2d.font_bold,
                0,
            );
            d2d.draw_text(
                artist,
                bx + 14.0,
                by + 34.0,
                bw - 140.0,
                20.0,
                TEXT_SECONDARY,
                &d2d.font_small,
                0,
            );

            let src_r = Rect {
                x: bx + bw - 118.0,
                y: by + 10.0,
                w: 104.0,
                h: 24.0,
            };
            d2d.fill_round_rect(src_r.x, src_r.y, src_r.w, src_r.h, 8.0, SURFACE_HOVER);
            d2d.draw_text(
                "Switch Player",
                src_r.x,
                src_r.y,
                src_r.w,
                src_r.h,
                SKY,
                &d2d.font_small,
                1,
            );
            hits.push(src_r, UiAction::MediaCycleSource);

            let lyr_r = Rect {
                x: bx + bw - 118.0,
                y: by + 38.0,
                w: 104.0,
                h: 24.0,
            };
            d2d.fill_round_rect(
                lyr_r.x,
                lyr_r.y,
                lyr_r.w,
                lyr_r.h,
                8.0,
                if data.show_lyrics {
                    TAB_ACTIVE
                } else {
                    SURFACE_HOVER
                },
            );
            d2d.draw_text(
                "Split Lyrics",
                lyr_r.x,
                lyr_r.y,
                lyr_r.w,
                lyr_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(lyr_r, UiAction::MediaToggleLyrics);

            let pos = data.media.live_position_secs();
            let dur = data.media.duration_secs.max(1.0);
            draw_slider(
                d2d,
                Rect {
                    x: bx + 14.0,
                    y: by + 68.0,
                    w: bw - 28.0,
                    h: 16.0,
                },
                (pos / dur) as f32,
                SKY,
                0,
                0,
                hits,
            );

            let ctrl_y = by + 90.0;
            for (i, (lbl, act)) in [
                ("Prev", UiAction::MediaPrev),
                (
                    if data.media.is_playing {
                        "Pause"
                    } else {
                        "Play"
                    },
                    UiAction::MediaTogglePlay,
                ),
                ("Next", UiAction::MediaNext),
            ]
            .into_iter()
            .enumerate()
            {
                let r = Rect {
                    x: bx + 14.0 + (i as f32) * 86.0,
                    y: ctrl_y,
                    w: 78.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(r.x, r.y, r.w, r.h, 8.0, SURFACE_HOVER);
                d2d.draw_text(lbl, r.x, r.y, r.w, r.h, TEXT_PRIMARY, &d2d.font_small, 1);
                hits.push(r, act);
            }

            if data.show_lyrics && !data.lyrics.synced.is_empty() {
                let active = crate::modules::lyrics::active_line_index(&data.lyrics.synced, pos)
                    .unwrap_or(0);
                let start = active.saturating_sub(1);
                for (row, line) in data.lyrics.synced.iter().skip(start).take(4).enumerate() {
                    let lr = Rect {
                        x: bx + 14.0,
                        y: by + 128.0 + (row as f32) * 26.0,
                        w: bw - 28.0,
                        h: 24.0,
                    };
                    let is_cur = start + row == active;
                    if is_cur {
                        d2d.fill_round_rect(lr.x, lr.y, lr.w, lr.h, 6.0, TAB_ACTIVE);
                    }
                    d2d.draw_text(
                        &line.text,
                        lr.x + 8.0,
                        lr.y,
                        lr.w - 16.0,
                        lr.h,
                        if is_cur { TEXT_PRIMARY } else { TEXT_SECONDARY },
                        &d2d.font_small,
                        0,
                    );
                    hits.push(lr, UiAction::SeekLyric((line.time_secs * 1000.0) as u32));
                }
            } else {
                // Simple waveform fallback bars
                let bars = 24;
                let bar_w = (bw - 40.0) / (bars as f32);
                for i in 0..bars {
                    let amp = if data.media.is_playing {
                        8.0 + ((i * 7 + (pos as usize * 3)) % 28) as f32
                    } else {
                        6.0
                    };
                    d2d.fill_round_rect(
                        bx + 20.0 + (i as f32) * bar_w,
                        by + bh - 20.0 - amp,
                        bar_w - 4.0,
                        amp,
                        2.0,
                        SKY,
                    );
                }
            }
        }
        ExpandedTab::Audio => {
            d2d.fill_round_rect(bx, by, bw, bh, 12.0, SURFACE);
            d2d.draw_text(
                &format!(
                    "Master Volume: {}%",
                    (data.master_vol * 100.0).round() as i32
                ),
                bx + 14.0,
                by + 10.0,
                180.0,
                22.0,
                TEXT_PRIMARY,
                &d2d.font_small,
                0,
            );
            let mute_r = Rect {
                x: bx + bw - 84.0,
                y: by + 10.0,
                w: 70.0,
                h: 24.0,
            };
            d2d.fill_round_rect(
                mute_r.x,
                mute_r.y,
                mute_r.w,
                mute_r.h,
                8.0,
                if data.master_muted {
                    ROSE
                } else {
                    SURFACE_HOVER
                },
            );
            d2d.draw_text(
                if data.master_muted { "Muted" } else { "Mute" },
                mute_r.x,
                mute_r.y,
                mute_r.w,
                mute_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(mute_r, UiAction::ToggleMasterMute);
            draw_slider(
                d2d,
                Rect {
                    x: bx + 14.0,
                    y: by + 36.0,
                    w: bw - 28.0,
                    h: 16.0,
                },
                data.master_vol,
                EMERALD,
                1,
                0,
                hits,
            );

            d2d.draw_text(
                &format!(
                    "Monitor Brightness (DDC/CI): {}%",
                    (data.brightness * 100.0).round() as i32
                ),
                bx + 14.0,
                by + 62.0,
                260.0,
                22.0,
                TEXT_PRIMARY,
                &d2d.font_small,
                0,
            );
            draw_slider(
                d2d,
                Rect {
                    x: bx + 14.0,
                    y: by + 86.0,
                    w: bw - 28.0,
                    h: 16.0,
                },
                data.brightness,
                AMBER,
                2,
                0,
                hits,
            );

            d2d.draw_text(
                "Per-App Mixer",
                bx + 14.0,
                by + 112.0,
                160.0,
                20.0,
                TEXT_MUTED,
                &d2d.font_small,
                0,
            );
            for (i, sess) in data.app_sessions.iter().take(4).enumerate() {
                let sy = by + 136.0 + (i as f32) * 26.0;
                d2d.draw_text(
                    &sess.name,
                    bx + 14.0,
                    sy,
                    140.0,
                    20.0,
                    TEXT_SECONDARY,
                    &d2d.font_small,
                    0,
                );
                draw_slider(
                    d2d,
                    Rect {
                        x: bx + 160.0,
                        y: sy + 2.0,
                        w: bw - 174.0,
                        h: 16.0,
                    },
                    sess.volume,
                    SKY,
                    3,
                    sess.pid,
                    hits,
                );
            }
        }
        ExpandedTab::Tasks => {
            for (i, flt) in [
                TaskFilter::Today,
                TaskFilter::Upcoming,
                TaskFilter::Completed,
            ]
            .into_iter()
            .enumerate()
            {
                let r = Rect {
                    x: bx + (i as f32) * 86.0,
                    y: by,
                    w: 80.0,
                    h: 26.0,
                };
                d2d.fill_round_rect(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    8.0,
                    if data.task_filter == flt {
                        TAB_ACTIVE
                    } else {
                        SURFACE
                    },
                );
                d2d.draw_text(
                    match flt {
                        TaskFilter::Today => "Today",
                        TaskFilter::Upcoming => "Upcoming",
                        TaskFilter::Completed => "Done",
                    },
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    TEXT_PRIMARY,
                    &d2d.font_small,
                    1,
                );
                hits.push(r, UiAction::SetTaskFilter(flt));
            }
            draw_input(
                d2d,
                Rect {
                    x: bx,
                    y: by + 34.0,
                    w: bw - 68.0,
                    h: 30.0,
                },
                data.task_input,
                "Add task (e.g. Call Alex 15:30)...",
                data.focused_input == Some(1),
                1,
                hits,
            );
            let add_r = Rect {
                x: bx + bw - 62.0,
                y: by + 34.0,
                w: 62.0,
                h: 30.0,
            };
            d2d.fill_round_rect(add_r.x, add_r.y, add_r.w, add_r.h, 8.0, TAB_ACTIVE);
            d2d.draw_text(
                "Add",
                add_r.x,
                add_r.y,
                add_r.w,
                add_r.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(add_r, UiAction::AddTask);

            for (i, t) in data.tasks.iter().take(5).enumerate() {
                let ry = by + 72.0 + (i as f32) * 32.0;
                let row = Rect {
                    x: bx,
                    y: ry,
                    w: bw - 38.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(row.x, row.y, row.w, row.h, 8.0, SURFACE);
                let lbl = if t.due_label.is_empty() {
                    t.title.clone()
                } else {
                    format!("{}  ({})", t.title, t.due_label)
                };
                d2d.draw_text(
                    &lbl,
                    row.x + 10.0,
                    row.y,
                    row.w - 20.0,
                    row.h,
                    if t.done { TEXT_MUTED } else { TEXT_PRIMARY },
                    &d2d.font_small,
                    0,
                );
                hits.push(row, UiAction::ToggleTask(t.id));

                let del = Rect {
                    x: bx + bw - 32.0,
                    y: ry,
                    w: 32.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(del.x, del.y, del.w, del.h, 8.0, SURFACE_HOVER);
                d2d.draw_text("x", del.x, del.y, del.w, del.h, ROSE, &d2d.font_small, 1);
                hits.push(del, UiAction::DeleteTask(t.id));
            }
        }
        ExpandedTab::Calendar => {
            d2d.fill_round_rect(bx, by, bw, bh, 12.0, SURFACE);
            d2d.draw_text(
                "Today & Upcoming Agenda",
                bx + 14.0,
                by + 10.0,
                bw - 28.0,
                22.0,
                SKY,
                &d2d.font_bold,
                0,
            );
            if data.agenda.is_empty() {
                d2d.draw_text(
                    "No scheduled tasks or .ics events",
                    bx + 14.0,
                    by + 44.0,
                    bw - 28.0,
                    24.0,
                    TEXT_MUTED,
                    &d2d.font_small,
                    0,
                );
            } else {
                for (i, ag) in data.agenda.iter().take(6).enumerate() {
                    let ay = by + 40.0 + (i as f32) * 30.0;
                    d2d.draw_text(
                        &format!("[{}] {} — {}", ag.source, ag.time_label, ag.title),
                        bx + 14.0,
                        ay,
                        bw - 28.0,
                        24.0,
                        TEXT_PRIMARY,
                        &d2d.font_small,
                        0,
                    );
                }
            }
        }
        ExpandedTab::Timer => {
            d2d.fill_round_rect(bx, by, bw, bh, 12.0, SURFACE);
            d2d.draw_text(
                &format!(
                    "{} • Round {}/{}",
                    data.timer.label, data.timer.round, data.timer.total_rounds
                ),
                bx + 16.0,
                by + 14.0,
                bw - 32.0,
                22.0,
                SKY,
                &d2d.font_small,
                1,
            );
            d2d.draw_text(
                &data.timer.formatted_remaining(),
                bx + 16.0,
                by + 48.0,
                bw - 32.0,
                44.0,
                TEXT_PRIMARY,
                &d2d.font_mono,
                1,
            );
            d2d.draw_text(
                &format!("Until {}", data.timer.until_done_clock()),
                bx + 16.0,
                by + 96.0,
                bw - 32.0,
                20.0,
                TEXT_SECONDARY,
                &d2d.font_small,
                1,
            );

            let cy = by + 132.0;
            for (i, (lbl, act)) in [
                (
                    if data.timer.running { "Pause" } else { "Start" },
                    UiAction::TimerToggle,
                ),
                ("Reset", UiAction::TimerReset),
                ("25m", UiAction::TimerPreset(1500)),
                ("5m", UiAction::TimerPreset(300)),
                ("15m", UiAction::TimerPreset(900)),
            ]
            .into_iter()
            .enumerate()
            {
                let r = Rect {
                    x: bx + 24.0 + (i as f32) * 96.0,
                    y: cy,
                    w: 86.0,
                    h: 32.0,
                };
                d2d.fill_round_rect(r.x, r.y, r.w, r.h, 8.0, SURFACE_HOVER);
                d2d.draw_text(lbl, r.x, r.y, r.w, r.h, TEXT_PRIMARY, &d2d.font_small, 1);
                hits.push(r, act);
            }
        }
        ExpandedTab::Apps => {
            draw_input(
                d2d,
                Rect {
                    x: bx,
                    y: by,
                    w: bw - 68.0,
                    h: 30.0,
                },
                data.app_input,
                "Pin .exe path, folder, or URL...",
                data.focused_input == Some(2),
                2,
                hits,
            );
            let pin_b = Rect {
                x: bx + bw - 62.0,
                y: by,
                w: 62.0,
                h: 30.0,
            };
            d2d.fill_round_rect(pin_b.x, pin_b.y, pin_b.w, pin_b.h, 8.0, TAB_ACTIVE);
            d2d.draw_text(
                "Pin",
                pin_b.x,
                pin_b.y,
                pin_b.w,
                pin_b.h,
                TEXT_PRIMARY,
                &d2d.font_small,
                1,
            );
            hits.push(pin_b, UiAction::AddShortcut);

            for (i, sc) in data.shortcuts.iter().take(6).enumerate() {
                let ry = by + 40.0 + (i as f32) * 32.0;
                let row = Rect {
                    x: bx,
                    y: ry,
                    w: bw - 38.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(row.x, row.y, row.w, row.h, 8.0, SURFACE);
                d2d.draw_text(
                    &format!("{}  —  {}", sc.name, sc.target),
                    row.x + 10.0,
                    row.y,
                    row.w - 20.0,
                    row.h,
                    TEXT_PRIMARY,
                    &d2d.font_small,
                    0,
                );
                hits.push(row, UiAction::LaunchShortcut(i));

                let del = Rect {
                    x: bx + bw - 32.0,
                    y: ry,
                    w: 32.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(del.x, del.y, del.w, del.h, 8.0, SURFACE_HOVER);
                d2d.draw_text("x", del.x, del.y, del.w, del.h, ROSE, &d2d.font_small, 1);
                hits.push(del, UiAction::RemoveShortcut(sc.id));
            }
        }
        ExpandedTab::Shelf => {
            let col_w = (bw - 10.0) * 0.5;
            d2d.fill_round_rect(bx, by, col_w, bh, 12.0, SURFACE);
            d2d.draw_text(
                "File Shelf (Drop files)",
                bx + 10.0,
                by + 8.0,
                col_w - 66.0,
                20.0,
                SKY,
                &d2d.font_small,
                0,
            );
            let clr = Rect {
                x: bx + col_w - 54.0,
                y: by + 6.0,
                w: 46.0,
                h: 22.0,
            };
            d2d.fill_round_rect(clr.x, clr.y, clr.w, clr.h, 6.0, SURFACE_HOVER);
            d2d.draw_text(
                "Clear",
                clr.x,
                clr.y,
                clr.w,
                clr.h,
                TEXT_SECONDARY,
                &d2d.font_small,
                1,
            );
            hits.push(clr, UiAction::ClearShelf);

            for (i, it) in data.shelf.iter().take(5).enumerate() {
                let r = Rect {
                    x: bx + 8.0,
                    y: by + 34.0 + (i as f32) * 32.0,
                    w: col_w - 42.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(r.x, r.y, r.w, r.h, 7.0, SURFACE_HOVER);
                d2d.draw_text(
                    &format!("{} ({})", it.name, it.size_label),
                    r.x + 8.0,
                    r.y,
                    r.w - 16.0,
                    r.h,
                    TEXT_PRIMARY,
                    &d2d.font_small,
                    0,
                );
                hits.push(r, UiAction::OpenShelfItem(i));
                let rm = Rect {
                    x: bx + col_w - 30.0,
                    y: r.y,
                    w: 22.0,
                    h: 28.0,
                };
                d2d.draw_text("x", rm.x, rm.y, rm.w, rm.h, ROSE, &d2d.font_small, 1);
                hits.push(rm, UiAction::RemoveShelfItem(i));
            }

            let rx = bx + col_w + 10.0;
            d2d.fill_round_rect(rx, by, col_w, bh, 12.0, SURFACE);
            d2d.draw_text(
                "Clipboard (Privacy-Safe)",
                rx + 10.0,
                by + 8.0,
                col_w - 20.0,
                20.0,
                EMERALD,
                &d2d.font_small,
                0,
            );
            for (i, c) in data.clips.iter().take(5).enumerate() {
                let r = Rect {
                    x: rx + 8.0,
                    y: by + 34.0 + (i as f32) * 32.0,
                    w: col_w - 16.0,
                    h: 28.0,
                };
                d2d.fill_round_rect(r.x, r.y, r.w, r.h, 7.0, SURFACE_HOVER);
                d2d.draw_text(
                    &c.preview,
                    r.x + 8.0,
                    r.y,
                    r.w - 16.0,
                    r.h,
                    TEXT_PRIMARY,
                    &d2d.font_small,
                    0,
                );
                hits.push(r, UiAction::CopyClip(i));
            }
        }
    }
}
