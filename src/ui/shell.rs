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
    rgba, AMBER, BG_BOTTOM, BG_MINIMAL, BG_TOP, BORDER_SPECULAR, BORDER_SUBTLE, CARD_BG,
    DISCORD_BLURPLE, EMERALD, ICON_ON_WHITE, PILL_WHITE, ROSE, SKY, SURFACE, SURFACE_HOVER,
    TAB_ACTIVE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, TRACK_BG,
};
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

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

/// Returns Monday=0 .. Sunday=6
fn local_weekday_mon0() -> usize {
    unsafe {
        let st = GetLocalTime();
        // wDayOfWeek: Sunday=0, Monday=1 .. Saturday=6
        ((st.wDayOfWeek as usize) + 6) % 7
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

fn album_accent_color(seed: &str) -> D2D1_COLOR_F {
    let hash = seed
        .bytes()
        .fold(5381u32, |h, b| h.wrapping_mul(33).wrapping_add(b as u32));
    match hash % 5 {
        0 => SKY,
        1 => EMERALD,
        2 => DISCORD_BLURPLE,
        3 => AMBER,
        _ => ROSE,
    }
}

// ============================================================================
// Direct2D Vector Icon Primitives
// ============================================================================

fn draw_icon_play(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.fill_triangle(
        (cx - r * 0.42, cy - r * 0.68),
        (cx - r * 0.42, cy + r * 0.68),
        (cx + r * 0.72, cy),
        col,
    );
}

fn draw_icon_pause(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    let bw = (r * 0.34).max(2.0);
    let bh = r * 1.25;
    d2d.fill_round_rect(cx - r * 0.52, cy - bh * 0.5, bw, bh, 1.2, col);
    d2d.fill_round_rect(cx + r * 0.18, cy - bh * 0.5, bw, bh, 1.2, col);
}

fn draw_icon_prev(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.fill_round_rect(cx - r * 0.75, cy - r * 0.6, 2.0, r * 1.2, 1.0, col);
    d2d.fill_triangle(
        (cx + r * 0.65, cy - r * 0.6),
        (cx + r * 0.65, cy + r * 0.6),
        (cx - r * 0.45, cy),
        col,
    );
}

fn draw_icon_next(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.fill_triangle(
        (cx - r * 0.65, cy - r * 0.6),
        (cx - r * 0.65, cy + r * 0.6),
        (cx + r * 0.45, cy),
        col,
    );
    d2d.fill_round_rect(cx + r * 0.55, cy - r * 0.6, 2.0, r * 1.2, 1.0, col);
}

fn draw_icon_search(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    let cr = r * 0.55;
    d2d.draw_circle(cx - r * 0.15, cy - r * 0.15, cr, col, 1.6);
    d2d.draw_line(
        cx + cr * 0.45,
        cy + cr * 0.45,
        cx + r * 0.75,
        cy + r * 0.75,
        col,
        1.7,
    );
}

fn draw_icon_pin(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F, pinned: bool) {
    if pinned {
        d2d.fill_circle(cx, cy - r * 0.25, r * 0.42, col);
    } else {
        d2d.draw_circle(cx, cy - r * 0.25, r * 0.42, col, 1.5);
    }
    d2d.draw_line(cx, cy + r * 0.17, cx, cy + r * 0.78, col, 1.6);
}

fn draw_icon_chevron_up(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_polyline(
        &[
            (cx - r * 0.65, cy + r * 0.28),
            (cx, cy - r * 0.35),
            (cx + r * 0.65, cy + r * 0.28),
        ],
        col,
        1.7,
    );
}

fn draw_icon_mic(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F, muted: bool) {
    d2d.fill_round_rect(
        cx - r * 0.28,
        cy - r * 0.72,
        r * 0.56,
        r * 0.92,
        r * 0.28,
        col,
    );
    d2d.draw_arc_ring(cx, cy - r * 0.12, r * 0.56, 15.0, 150.0, col, 1.5);
    d2d.draw_line(cx, cy + r * 0.44, cx, cy + r * 0.78, col, 1.5);
    if muted {
        d2d.draw_line(
            cx - r * 0.72,
            cy - r * 0.65,
            cx + r * 0.72,
            cy + r * 0.65,
            ROSE,
            1.8,
        );
    }
}

fn draw_icon_headphones(
    d2d: &D2dRenderer,
    cx: f32,
    cy: f32,
    r: f32,
    col: D2D1_COLOR_F,
    deafened: bool,
) {
    d2d.draw_arc_ring(cx, cy + r * 0.05, r * 0.65, 190.0, 160.0, col, 1.6);
    d2d.fill_round_rect(cx - r * 0.78, cy - r * 0.02, r * 0.32, r * 0.64, 2.0, col);
    d2d.fill_round_rect(cx + r * 0.46, cy - r * 0.02, r * 0.32, r * 0.64, 2.0, col);
    if deafened {
        d2d.draw_line(
            cx - r * 0.72,
            cy - r * 0.65,
            cx + r * 0.72,
            cy + r * 0.65,
            ROSE,
            1.8,
        );
    }
}

fn draw_icon_phone_off(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_arc_ring(cx, cy + r * 0.3, r * 0.7, 210.0, 120.0, col, 2.2);
    d2d.draw_line(
        cx - r * 0.65,
        cy - r * 0.45,
        cx + r * 0.65,
        cy + r * 0.55,
        col,
        1.6,
    );
}

fn draw_icon_speaker(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F, muted: bool) {
    d2d.fill_round_rect(cx - r * 0.75, cy - r * 0.32, r * 0.36, r * 0.64, 1.0, col);
    d2d.fill_triangle(
        (cx - r * 0.45, cy - r * 0.32),
        (cx - r * 0.45, cy + r * 0.32),
        (cx + r * 0.1, cy - r * 0.72),
        col,
    );
    d2d.fill_triangle(
        (cx - r * 0.45, cy + r * 0.32),
        (cx + r * 0.1, cy + r * 0.72),
        (cx + r * 0.1, cy - r * 0.72),
        col,
    );
    if muted {
        d2d.draw_line(
            cx + r * 0.3,
            cy - r * 0.35,
            cx + r * 0.8,
            cy + r * 0.35,
            ROSE,
            1.7,
        );
        d2d.draw_line(
            cx + r * 0.8,
            cy - r * 0.35,
            cx + r * 0.3,
            cy + r * 0.35,
            ROSE,
            1.7,
        );
    } else {
        d2d.draw_arc_ring(cx + r * 0.18, cy, r * 0.38, -50.0, 100.0, col, 1.4);
        d2d.draw_arc_ring(cx + r * 0.18, cy, r * 0.64, -50.0, 100.0, col, 1.4);
    }
}

fn draw_icon_sun(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.fill_circle(cx, cy, r * 0.38, col);
    for i in 0..8 {
        let a = (i as f32) * std::f32::consts::FRAC_PI_4;
        d2d.draw_line(
            cx + a.cos() * r * 0.56,
            cy + a.sin() * r * 0.56,
            cx + a.cos() * r * 0.82,
            cy + a.sin() * r * 0.82,
            col,
            1.4,
        );
    }
}

fn draw_icon_lyrics(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_line(
        cx - r * 0.65,
        cy - r * 0.45,
        cx + r * 0.65,
        cy - r * 0.45,
        col,
        1.7,
    );
    d2d.draw_line(cx - r * 0.65, cy, cx + r * 0.35, cy, col, 1.7);
    d2d.draw_line(
        cx - r * 0.65,
        cy + r * 0.45,
        cx + r * 0.52,
        cy + r * 0.45,
        col,
        1.7,
    );
}

fn draw_icon_check(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_polyline(
        &[
            (cx - r * 0.55, cy + r * 0.05),
            (cx - r * 0.12, cy + r * 0.48),
            (cx + r * 0.6, cy - r * 0.42),
        ],
        col,
        1.7,
    );
}

fn draw_icon_trash(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_line(
        cx - r * 0.55,
        cy - r * 0.45,
        cx + r * 0.55,
        cy - r * 0.45,
        col,
        1.5,
    );
    d2d.draw_line(
        cx - r * 0.2,
        cy - r * 0.65,
        cx + r * 0.2,
        cy - r * 0.65,
        col,
        1.5,
    );
    d2d.draw_round_rect(
        cx - r * 0.42,
        cy - r * 0.32,
        r * 0.84,
        r * 0.92,
        1.5,
        col,
        1.3,
    );
}

fn draw_icon_plus(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_line(cx - r * 0.6, cy, cx + r * 0.6, cy, col, 1.8);
    d2d.draw_line(cx, cy - r * 0.6, cx, cy + r * 0.6, col, 1.8);
}

fn draw_icon_eye(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_arc_ring(cx, cy + r * 0.32, r * 0.85, 225.0, 90.0, col, 1.5);
    d2d.draw_arc_ring(cx, cy - r * 0.32, r * 0.85, 45.0, 90.0, col, 1.5);
    d2d.fill_circle(cx, cy, r * 0.24, col);
}

fn draw_icon_shield(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_polyline(
        &[
            (cx, cy - r * 0.72),
            (cx + r * 0.62, cy - r * 0.44),
            (cx + r * 0.52, cy + r * 0.25),
            (cx, cy + r * 0.75),
            (cx - r * 0.52, cy + r * 0.25),
            (cx - r * 0.62, cy - r * 0.44),
            (cx, cy - r * 0.72),
        ],
        col,
        1.5,
    );
}

fn draw_icon_layers(d2d: &D2dRenderer, cx: f32, cy: f32, r: f32, col: D2D1_COLOR_F) {
    d2d.draw_round_rect(
        cx - r * 0.65,
        cy - r * 0.55,
        r * 1.3,
        r * 0.75,
        2.0,
        col,
        1.4,
    );
    d2d.draw_line(
        cx - r * 0.5,
        cy + r * 0.45,
        cx + r * 0.5,
        cy + r * 0.45,
        col,
        1.4,
    );
}

// ============================================================================
// Main Shell Renderer
// ============================================================================

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
            let dot_col = if data.media.is_playing {
                EMERALD
            } else if data.discord.in_call {
                DISCORD_BLURPLE
            } else {
                SKY
            };
            d2d.fill_circle(x + w * 0.5, y + h * 0.5, 2.2, dot_col);
            hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
            return;
        }
        NotchStyle::Minimal => {
            d2d.draw_top_edge_notch(x, y, w, h, r, 10.0, BG_TOP, BG_BOTTOM, BORDER_SPECULAR);
        }
    }

    match sm.state {
        NotchState::Collapsed => draw_collapsed(d2d, sm, cfg, data, x, y, w, h, hits),
        NotchState::Peek => draw_peek(d2d, sm, x, y, w, h, hits),
        NotchState::Expanded => draw_expanded(d2d, sm, cfg, data, x, y, w, h, hits),
        NotchState::Hidden => {}
    }
}

fn draw_collapsed(
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
    let has_live = data.media.is_playing || data.timer.running || data.discord.in_call;

    // Left: crisp HH:MM clock
    d2d.draw_text(
        &local_clock_hhmm(),
        x + 16.0,
        y,
        56.0,
        h,
        TEXT_PRIMARY,
        &d2d.font_bold,
        0,
    );

    // Center: Nusa Mascot eyes when idle
    if cfg.mascot_enabled && !has_live {
        draw_mascot_eyes(d2d, x + w * 0.5, y + h * 0.5, sm.mascot_mood);
    }

    // Right: Ranked Live Activity (Timer > Music EQ > Discord > Battery Ring)
    if data.timer.running {
        let frac = (data.timer.remaining_secs as f32 / (data.timer.total_secs.max(1) as f32))
            .clamp(0.0, 1.0);
        let rcx = x + w - 66.0;
        let rcy = y + h * 0.5;
        d2d.draw_circle(rcx, rcy, 7.5, TRACK_BG, 2.0);
        d2d.draw_arc_ring(rcx, rcy, 7.5, -90.0, frac * 360.0, SKY, 2.0);
        d2d.draw_text(
            &data.timer.formatted_remaining(),
            x + w - 55.0,
            y,
            42.0,
            h,
            SKY,
            &d2d.font_small,
            2,
        );
    } else if data.media.is_playing {
        let phase = sm.anim_clock;
        for i in 0..4 {
            let amp = 4.0 + ((phase * 5.2 + (i as f32) * 1.15).sin().abs()) * 11.0;
            let bx = x + w - 34.0 + (i as f32) * 4.6;
            d2d.fill_round_rect(bx, y + (h - amp) * 0.5, 2.6, amp, 1.3, EMERALD);
        }
    } else if data.discord.discord_running {
        let dc = if data.discord.muted {
            ROSE
        } else if data.discord.in_call {
            EMERALD
        } else {
            DISCORD_BLURPLE
        };
        d2d.fill_circle(x + w - 22.0, y + h * 0.5, 4.2, dc);
    } else if let Some((pct, charging)) = battery_percent() {
        let ring_col = if charging {
            EMERALD
        } else if pct <= 20 {
            ROSE
        } else {
            SKY
        };
        let rcx = x + w - 50.0;
        let rcy = y + h * 0.5;
        d2d.draw_circle(rcx, rcy, 7.0, SURFACE_HOVER, 1.8);
        d2d.draw_arc_ring(
            rcx,
            rcy,
            7.0,
            -90.0,
            (pct as f32 / 100.0) * 360.0,
            ring_col,
            1.8,
        );
        d2d.draw_text(
            &format!("{pct}%"),
            x + w - 40.0,
            y,
            28.0,
            h,
            TEXT_SECONDARY,
            &d2d.font_small,
            2,
        );
    }

    hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
}

fn draw_mascot_eyes(d2d: &D2dRenderer, cx: f32, cy: f32, mood: MascotMood) {
    let offset = 7.5;
    match mood {
        MascotMood::Sleepy => {
            d2d.draw_arc_ring(cx - offset, cy - 1.0, 4.0, 15.0, 150.0, TEXT_MUTED, 1.6);
            d2d.draw_arc_ring(cx + offset, cy - 1.0, 4.0, 15.0, 150.0, TEXT_MUTED, 1.6);
        }
        MascotMood::Blink => {
            d2d.draw_line(cx - offset - 3.5, cy, cx - offset + 3.5, cy, SKY, 1.8);
            d2d.draw_line(cx + offset - 3.5, cy, cx + offset + 3.5, cy, SKY, 1.8);
        }
        MascotMood::Happy => {
            d2d.draw_arc_ring(cx - offset, cy + 1.5, 4.2, 195.0, 150.0, EMERALD, 1.8);
            d2d.draw_arc_ring(cx + offset, cy + 1.5, 4.2, 195.0, 150.0, EMERALD, 1.8);
        }
        MascotMood::Awake => {
            d2d.fill_circle(cx - offset, cy, 3.6, SKY);
            d2d.fill_circle(cx + offset, cy, 3.6, SKY);
            d2d.fill_circle(cx - offset + 1.1, cy - 1.1, 1.2, PILL_WHITE);
            d2d.fill_circle(cx + offset + 1.1, cy - 1.1, 1.2, PILL_WHITE);
        }
    }
}

fn draw_peek(
    d2d: &D2dRenderer,
    sm: &NotchStateMachine,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    hits: &mut UiHitMap,
) {
    if let Some(p) = &sm.active_peek {
        let badge_col = match p.accent {
            0 => SKY,
            1 => EMERALD,
            2 => AMBER,
            _ => ROSE,
        };
        let bx = x + 22.0;
        let by = y + h * 0.5;
        d2d.fill_circle(bx, by, 14.0, rgba(255, 255, 255, 0.08));
        d2d.draw_circle(bx, by, 14.0, badge_col, 1.5);
        d2d.fill_circle(bx, by, 4.5, badge_col);

        d2d.draw_text(
            &p.title,
            x + 46.0,
            y + 9.0,
            w - 64.0,
            20.0,
            TEXT_PRIMARY,
            &d2d.font_bold,
            0,
        );
        d2d.draw_text(
            &p.subtitle,
            x + 46.0,
            y + 28.0,
            w - 64.0,
            18.0,
            TEXT_SECONDARY,
            &d2d.font_small,
            0,
        );
    }
    hits.push(Rect { x, y, w, h }, UiAction::ToggleExpand);
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
    let pad = 16.0;
    let ix = x + pad;
    let iw = w - pad * 2.0;

    // ------------------------------------------------------------------------
    // 1. Segmented Pill Header + Pin / Collapse Vector Buttons
    // ------------------------------------------------------------------------
    let tabs = [
        (ExpandedTab::Home, "Home"),
        (ExpandedTab::Music, "Music"),
        (ExpandedTab::Hud, "HUD"),
        (ExpandedTab::Tasks, "Tasks"),
        (ExpandedTab::Timer, "Timer"),
        (ExpandedTab::Calendar, "Cal"),
        (ExpandedTab::Apps, "Apps"),
        (ExpandedTab::Shelf, "Shelf"),
    ];

    let track_w = iw - 62.0;
    let track_h = 28.0;
    let track_y = y + 12.0;
    d2d.fill_round_rect(ix, track_y, track_w, track_h, 14.0, TRACK_BG);
    d2d.draw_round_rect(ix, track_y, track_w, track_h, 14.0, BORDER_SUBTLE, 1.0);

    let tab_w = (track_w - 6.0) / (tabs.len() as f32);
    for (idx, (tab, label)) in tabs.iter().enumerate() {
        let r = Rect {
            x: ix + 3.0 + (idx as f32) * tab_w,
            y: track_y + 3.0,
            w: tab_w - 2.0,
            h: track_h - 6.0,
        };
        let active = sm.tab == *tab;
        if active {
            d2d.fill_round_rect(r.x, r.y, r.w, r.h, 11.0, TAB_ACTIVE);
            d2d.draw_round_rect(r.x, r.y, r.w, r.h, 11.0, BORDER_SPECULAR, 0.8);
        }
        d2d.draw_text(
            label,
            r.x,
            r.y,
            r.w,
            r.h,
            if active { TEXT_PRIMARY } else { TEXT_MUTED },
            &d2d.font_small,
            1,
        );
        hits.push(r, UiAction::SelectTab(*tab));
    }

    let pin_r = Rect {
        x: x + w - pad - 54.0,
        y: track_y + 2.0,
        w: 24.0,
        h: 24.0,
    };
    d2d.fill_circle(
        pin_r.x + 12.0,
        pin_r.y + 12.0,
        12.0,
        if sm.pinned {
            rgba(96, 165, 250, 0.22)
        } else {
            TRACK_BG
        },
    );
    draw_icon_pin(
        d2d,
        pin_r.x + 12.0,
        pin_r.y + 12.0,
        6.5,
        if sm.pinned { SKY } else { TEXT_SECONDARY },
        sm.pinned,
    );
    hits.push(pin_r, UiAction::TogglePin);

    let cls_r = Rect {
        x: x + w - pad - 24.0,
        y: track_y + 2.0,
        w: 24.0,
        h: 24.0,
    };
    d2d.fill_circle(cls_r.x + 12.0, cls_r.y + 12.0, 12.0, TRACK_BG);
    draw_icon_chevron_up(d2d, cls_r.x + 12.0, cls_r.y + 12.0, 6.5, TEXT_SECONDARY);
    hits.push(cls_r, UiAction::Collapse);

    // Body region
    let bx = ix;
    let by = track_y + track_h + 12.0;
    let bw = iw;
    let bh = (y + h - 14.0 - by).max(120.0);

    match sm.tab {
        ExpandedTab::Home => {
            draw_home_tab(d2d, cfg, data, bx, by, bw, bh, hits);
        }
        ExpandedTab::Music => {
            draw_music_tab(d2d, sm, data, bx, by, bw, bh, hits);
        }
        ExpandedTab::Hud => {
            draw_hud_tab(d2d, data, bx, by, bw, hits);
        }
        ExpandedTab::Tasks => {
            draw_tasks_tab(d2d, data, bx, by, bw, hits);
        }
        ExpandedTab::Calendar => {
            draw_calendar_tab(d2d, data, bx, by, bw, bh);
        }
        ExpandedTab::Timer => {
            draw_timer_tab(d2d, data, bx, by, bw, bh, hits);
        }
        ExpandedTab::Apps => {
            draw_apps_tab(d2d, data, bx, by, bw, hits);
        }
        ExpandedTab::Shelf => {
            draw_shelf_tab(d2d, data, bx, by, bw, bh, hits);
        }
    }
}

fn draw_home_tab(
    d2d: &D2dRenderer,
    cfg: &AppConfig,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    hits: &mut UiHitMap,
) {
    // 1. Spotlight Google Search Pill
    let sr = Rect {
        x: bx,
        y: by,
        w: bw,
        h: 34.0,
    };
    let focused = data.focused_input == Some(0);
    d2d.fill_round_rect(sr.x, sr.y, sr.w, sr.h, 17.0, TRACK_BG);
    d2d.draw_round_rect(
        sr.x,
        sr.y,
        sr.w,
        sr.h,
        17.0,
        if focused { SKY } else { BORDER_SUBTLE },
        if focused { 1.4 } else { 1.0 },
    );
    draw_icon_search(
        d2d,
        sr.x + 17.0,
        sr.y + 17.0,
        6.5,
        if focused { SKY } else { TEXT_MUTED },
    );
    let stext = if data.search_text.is_empty() {
        "Search Google..."
    } else {
        data.search_text
    };
    d2d.draw_text(
        stext,
        sr.x + 32.0,
        sr.y,
        sr.w - 134.0,
        sr.h,
        if data.search_text.is_empty() {
            TEXT_MUTED
        } else {
            TEXT_PRIMARY
        },
        &d2d.font_small,
        0,
    );
    // Keycap badge
    let kcap = Rect {
        x: sr.x + sr.w - 94.0,
        y: sr.y + 7.0,
        w: 84.0,
        h: 20.0,
    };
    d2d.fill_round_rect(kcap.x, kcap.y, kcap.w, kcap.h, 6.0, CARD_BG);
    d2d.draw_text(
        "Ctrl+Alt+Space",
        kcap.x,
        kcap.y,
        kcap.w,
        kcap.h,
        TEXT_MUTED,
        &d2d.font_small,
        1,
    );
    hits.push(sr, UiAction::FocusInput(0));

    // 2. 12-Column Bento Row: Left 7 cols (Week Strip + Next Agenda) | Right 5 cols (Discord Voice + Battery)
    let mid_y = by + 44.0;
    let mid_h = (bh - 98.0).clamp(88.0, 112.0);
    let left_w = bw * 0.56;
    let right_w = bw - left_w - 10.0;
    let rx = bx + left_w + 10.0;

    // Left Card: Week Strip + Today Agenda
    d2d.fill_round_rect(bx, mid_y, left_w, mid_h, 14.0, CARD_BG);
    d2d.draw_round_rect(bx, mid_y, left_w, mid_h, 14.0, BORDER_SUBTLE, 1.0);

    let today_idx = local_weekday_mon0();
    let days = ["M", "T", "W", "T", "F", "S", "S"];
    let cell_w = (left_w - 24.0) / 7.0;
    for (i, d) in days.iter().enumerate() {
        let cx = bx + 12.0 + (i as f32) * cell_w;
        let cy = mid_y + 10.0;
        let is_today = i == today_idx;
        if is_today {
            d2d.fill_round_rect(cx + 2.0, cy, cell_w - 4.0, 22.0, 11.0, PILL_WHITE);
        }
        d2d.draw_text(
            d,
            cx,
            cy,
            cell_w,
            22.0,
            if is_today { ICON_ON_WHITE } else { TEXT_MUTED },
            &d2d.font_small,
            1,
        );
    }

    if let Some(next_ag) = data.agenda.first() {
        d2d.draw_text(
            &format!("{}  •  {}", next_ag.time_label, next_ag.title),
            bx + 14.0,
            mid_y + 42.0,
            left_w - 28.0,
            20.0,
            TEXT_PRIMARY,
            &d2d.font_small,
            0,
        );
        d2d.draw_text(
            &format!("Source: {}", next_ag.source),
            bx + 14.0,
            mid_y + 64.0,
            left_w - 28.0,
            18.0,
            SKY,
            &d2d.font_small,
            0,
        );
    } else {
        d2d.draw_text(
            "No upcoming events today",
            bx + 14.0,
            mid_y + 44.0,
            left_w - 28.0,
            20.0,
            TEXT_SECONDARY,
            &d2d.font_small,
            0,
        );
        d2d.draw_text(
            "Tasks & .ics calendar sync automatically",
            bx + 14.0,
            mid_y + 66.0,
            left_w - 28.0,
            18.0,
            TEXT_MUTED,
            &d2d.font_small,
            0,
        );
    }

    // Right Card: Discord Voice Controls
    d2d.fill_round_rect(rx, mid_y, right_w, mid_h, 14.0, CARD_BG);
    d2d.draw_round_rect(rx, mid_y, right_w, mid_h, 14.0, BORDER_SUBTLE, 1.0);
    let dc_col = if data.discord.discord_running {
        if data.discord.muted {
            ROSE
        } else {
            EMERALD
        }
    } else {
        TEXT_MUTED
    };
    d2d.fill_circle(rx + 18.0, mid_y + 20.0, 4.0, dc_col);
    let dc_label = if data.discord.discord_running {
        "Discord Voice"
    } else {
        "Discord Idle"
    };
    d2d.draw_text(
        dc_label,
        rx + 28.0,
        mid_y + 10.0,
        right_w - 40.0,
        20.0,
        TEXT_PRIMARY,
        &d2d.font_small,
        0,
    );

    let btn_y = mid_y + 42.0;
    let gap = 8.0;
    let btn_w = (right_w - 24.0 - gap * 2.0) / 3.0;
    let m_rect = Rect {
        x: rx + 12.0,
        y: btn_y,
        w: btn_w,
        h: 34.0,
    };
    d2d.fill_round_rect(
        m_rect.x,
        m_rect.y,
        m_rect.w,
        m_rect.h,
        10.0,
        if data.discord.muted {
            rgba(251, 113, 133, 0.22)
        } else {
            SURFACE_HOVER
        },
    );
    draw_icon_mic(
        d2d,
        m_rect.x + m_rect.w * 0.5,
        m_rect.y + m_rect.h * 0.5,
        7.5,
        if data.discord.muted {
            ROSE
        } else {
            TEXT_PRIMARY
        },
        data.discord.muted,
    );
    hits.push(m_rect, UiAction::DiscordMute);

    let d_rect = Rect {
        x: m_rect.x + btn_w + gap,
        y: btn_y,
        w: btn_w,
        h: 34.0,
    };
    d2d.fill_round_rect(
        d_rect.x,
        d_rect.y,
        d_rect.w,
        d_rect.h,
        10.0,
        if data.discord.deafened {
            rgba(251, 113, 133, 0.22)
        } else {
            SURFACE_HOVER
        },
    );
    draw_icon_headphones(
        d2d,
        d_rect.x + d_rect.w * 0.5,
        d_rect.y + d_rect.h * 0.5,
        7.5,
        if data.discord.deafened {
            ROSE
        } else {
            TEXT_PRIMARY
        },
        data.discord.deafened,
    );
    hits.push(d_rect, UiAction::DiscordDeafen);

    let h_rect = Rect {
        x: d_rect.x + btn_w + gap,
        y: btn_y,
        w: btn_w,
        h: 34.0,
    };
    d2d.fill_round_rect(
        h_rect.x,
        h_rect.y,
        h_rect.w,
        h_rect.h,
        10.0,
        rgba(251, 113, 133, 0.18),
    );
    draw_icon_phone_off(
        d2d,
        h_rect.x + h_rect.w * 0.5,
        h_rect.y + h_rect.h * 0.5,
        7.5,
        ROSE,
    );
    hits.push(h_rect, UiAction::DiscordDisconnect);

    // 3. Bottom Quick Control Bento Tiles
    let qy = mid_y + mid_h + 10.0;
    let qgap = 8.0;
    let qw = (bw - qgap * 3.0) / 4.0;
    let style_name = match cfg.style {
        NotchStyle::TopEdge => "Top-Edge",
        NotchStyle::FloatingPill => "Floating",
        NotchStyle::Minimal => "Minimal",
    };

    let tiles = [
        (0u8, "Style", style_name, true, UiAction::CycleStyle),
        (
            1u8,
            "Mascot",
            if cfg.mascot_enabled { "On" } else { "Off" },
            cfg.mascot_enabled,
            UiAction::ToggleMascot,
        ),
        (
            2u8,
            "Privacy",
            if cfg.hide_from_capture {
                "Hidden"
            } else {
                "Visible"
            },
            cfg.hide_from_capture,
            UiAction::TogglePrivacy,
        ),
        (3u8, "Peek", "Test Alert", false, UiAction::TriggerPeekDemo),
    ];

    for (i, (kind, title, val, active, act)) in tiles.into_iter().enumerate() {
        let tr = Rect {
            x: bx + (i as f32) * (qw + qgap),
            y: qy,
            w: qw,
            h: 40.0,
        };
        d2d.fill_round_rect(
            tr.x,
            tr.y,
            tr.w,
            tr.h,
            11.0,
            if active { SURFACE_HOVER } else { CARD_BG },
        );
        d2d.draw_round_rect(tr.x, tr.y, tr.w, tr.h, 11.0, BORDER_SUBTLE, 1.0);
        let icx = tr.x + 18.0;
        let icy = tr.y + 20.0;
        let icol = if active { SKY } else { TEXT_SECONDARY };
        match kind {
            0 => draw_icon_layers(d2d, icx, icy, 6.5, icol),
            1 => draw_icon_eye(d2d, icx, icy, 6.5, icol),
            2 => draw_icon_shield(d2d, icx, icy, 6.5, if active { EMERALD } else { icol }),
            _ => draw_icon_plus(d2d, icx, icy, 6.0, AMBER),
        }
        d2d.draw_text(
            title,
            tr.x + 32.0,
            tr.y + 4.0,
            tr.w - 38.0,
            16.0,
            TEXT_PRIMARY,
            &d2d.font_small,
            0,
        );
        d2d.draw_text(
            val,
            tr.x + 32.0,
            tr.y + 19.0,
            tr.w - 38.0,
            16.0,
            if active { SKY } else { TEXT_MUTED },
            &d2d.font_small,
            0,
        );
        hits.push(tr, act);
    }
}

fn draw_music_tab(
    d2d: &D2dRenderer,
    sm: &NotchStateMachine,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    hits: &mut UiHitMap,
) {
    let art_sz = 66.0;
    let accent = album_accent_color(&data.media.title);

    // Squircle album art card with ambient tint
    d2d.fill_round_rect(
        bx,
        by,
        art_sz,
        art_sz,
        14.0,
        rgba(
            (accent.r * 255.0) as u8,
            (accent.g * 255.0) as u8,
            (accent.b * 255.0) as u8,
            0.22,
        ),
    );
    d2d.draw_round_rect(bx, by, art_sz, art_sz, 14.0, accent, 1.0);
    d2d.draw_circle(bx + art_sz * 0.5, by + art_sz * 0.5, 16.0, accent, 1.8);
    d2d.fill_circle(bx + art_sz * 0.5, by + art_sz * 0.5, 5.0, PILL_WHITE);
    hits.push(
        Rect {
            x: bx,
            y: by,
            w: art_sz,
            h: art_sz,
        },
        UiAction::MediaCycleSource,
    );

    let tx = bx + art_sz + 14.0;
    let tw = bw - art_sz - 56.0;
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
        tx,
        by + 4.0,
        tw,
        24.0,
        TEXT_PRIMARY,
        &d2d.font_bold,
        0,
    );
    d2d.draw_text(
        artist,
        tx,
        by + 28.0,
        tw,
        18.0,
        TEXT_SECONDARY,
        &d2d.font_small,
        0,
    );

    let src_name = if data.media.source_app_id.is_empty() {
        "SMTC"
    } else {
        data.media
            .source_app_id
            .split('.')
            .next()
            .unwrap_or("Media")
    };
    d2d.draw_text(
        src_name,
        tx,
        by + 46.0,
        tw,
        16.0,
        accent,
        &d2d.font_small,
        0,
    );

    // Split Lyrics circular toggle button
    let lyr_btn = Rect {
        x: bx + bw - 34.0,
        y: by + 4.0,
        w: 34.0,
        h: 34.0,
    };
    d2d.fill_circle(
        lyr_btn.x + 17.0,
        lyr_btn.y + 17.0,
        17.0,
        if data.show_lyrics {
            rgba(96, 165, 250, 0.24)
        } else {
            CARD_BG
        },
    );
    draw_icon_lyrics(
        d2d,
        lyr_btn.x + 17.0,
        lyr_btn.y + 17.0,
        7.5,
        if data.show_lyrics {
            SKY
        } else {
            TEXT_SECONDARY
        },
    );
    hits.push(lyr_btn, UiAction::MediaToggleLyrics);

    // Scrubber + MM:SS / -MM:SS
    let pos = data.media.live_position_secs();
    let dur = data.media.duration_secs.max(1.0);
    let ratio = (pos / dur).clamp(0.0, 1.0) as f32;
    let rem = (dur - pos).max(0.0);

    let sy = by + 76.0;
    d2d.draw_text(
        &format!("{:02}:{:02}", (pos as u64) / 60, (pos as u64) % 60),
        bx,
        sy - 2.0,
        42.0,
        16.0,
        TEXT_MUTED,
        &d2d.font_small,
        0,
    );
    draw_slider(
        d2d,
        Rect {
            x: bx + 44.0,
            y: sy,
            w: bw - 92.0,
            h: 12.0,
        },
        ratio,
        PILL_WHITE,
        0,
        0,
        hits,
    );
    d2d.draw_text(
        &format!("-{:02}:{:02}", (rem as u64) / 60, (rem as u64) % 60),
        bx + bw - 46.0,
        sy - 2.0,
        46.0,
        16.0,
        TEXT_MUTED,
        &d2d.font_small,
        2,
    );

    // Transport Row: Prev | White Circular Play/Pause | Next
    let cy = by + 98.0;
    let mid_x = bx + bw * 0.5;
    let prev_r = Rect {
        x: mid_x - 66.0,
        y: cy + 4.0,
        w: 32.0,
        h: 32.0,
    };
    d2d.fill_circle(prev_r.x + 16.0, prev_r.y + 16.0, 16.0, CARD_BG);
    draw_icon_prev(d2d, prev_r.x + 16.0, prev_r.y + 16.0, 7.0, TEXT_PRIMARY);
    hits.push(prev_r, UiAction::MediaPrev);

    let play_r = Rect {
        x: mid_x - 20.0,
        y: cy,
        w: 40.0,
        h: 40.0,
    };
    d2d.fill_circle(play_r.x + 20.0, play_r.y + 20.0, 20.0, PILL_WHITE);
    if data.media.is_playing {
        draw_icon_pause(d2d, play_r.x + 20.0, play_r.y + 20.0, 8.5, ICON_ON_WHITE);
    } else {
        draw_icon_play(d2d, play_r.x + 21.0, play_r.y + 20.0, 8.5, ICON_ON_WHITE);
    }
    hits.push(play_r, UiAction::MediaTogglePlay);

    let next_r = Rect {
        x: mid_x + 34.0,
        y: cy + 4.0,
        w: 32.0,
        h: 32.0,
    };
    d2d.fill_circle(next_r.x + 16.0, next_r.y + 16.0, 16.0, CARD_BG);
    draw_icon_next(d2d, next_r.x + 16.0, next_r.y + 16.0, 7.0, TEXT_PRIMARY);
    hits.push(next_r, UiAction::MediaNext);

    // Split Lyrics OR Spectrum Visualizer
    let ly = cy + 46.0;
    let lh = (by + bh - ly).max(40.0);
    if data.show_lyrics && !data.lyrics.synced.is_empty() {
        d2d.fill_round_rect(bx, ly, bw, lh, 12.0, TRACK_BG);
        let active =
            crate::modules::lyrics::active_line_index(&data.lyrics.synced, pos).unwrap_or(0);
        let start = active.saturating_sub(1);
        for (row, line) in data.lyrics.synced.iter().skip(start).take(3).enumerate() {
            let lr = Rect {
                x: bx + 12.0,
                y: ly + 6.0 + (row as f32) * 20.0,
                w: bw - 24.0,
                h: 20.0,
            };
            let is_cur = start + row == active;
            d2d.draw_text(
                &line.text,
                lr.x,
                lr.y,
                lr.w,
                lr.h,
                if is_cur { TEXT_PRIMARY } else { TEXT_MUTED },
                if is_cur {
                    &d2d.font_bold
                } else {
                    &d2d.font_small
                },
                1,
            );
            hits.push(lr, UiAction::SeekLyric(line.time_ms));
        }
    } else if data.show_lyrics {
        d2d.fill_round_rect(bx, ly, bw, lh, 12.0, TRACK_BG);
        d2d.draw_text(
            "No synced .lrc found for this track",
            bx,
            ly,
            bw,
            lh,
            TEXT_MUTED,
            &d2d.font_small,
            1,
        );
    } else {
        d2d.fill_round_rect(bx, ly, bw, lh, 12.0, TRACK_BG);
        let bars = 24;
        let bar_sp = (bw - 32.0) / (bars as f32);
        for i in 0..bars {
            let amp = if data.media.is_playing {
                4.0 + ((sm.anim_clock * 4.8 + (i as f32) * 0.55).sin().abs()) * (lh - 16.0)
            } else {
                4.0
            };
            d2d.fill_round_rect(
                bx + 16.0 + (i as f32) * bar_sp,
                ly + (lh - amp) * 0.5,
                (bar_sp * 0.55).max(3.0),
                amp,
                2.0,
                if data.media.is_playing {
                    accent
                } else {
                    SURFACE_HOVER
                },
            );
        }
    }
}

fn draw_hud_tab(
    d2d: &D2dRenderer,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    hits: &mut UiHitMap,
) {
    // Thick iOS Control Center-style pill sliders
    let half_w = (bw - 12.0) * 0.5;

    // Master Volume Pill
    let vol_r = Rect {
        x: bx,
        y: by,
        w: half_w,
        h: 38.0,
    };
    draw_cc_slider(
        d2d,
        vol_r,
        data.master_vol,
        if data.master_muted { ROSE } else { SKY },
        1,
        0,
        hits,
    );
    let mute_hit = Rect {
        x: vol_r.x + 6.0,
        y: vol_r.y + 4.0,
        w: 30.0,
        h: 30.0,
    };
    draw_icon_speaker(
        d2d,
        mute_hit.x + 15.0,
        mute_hit.y + 15.0,
        7.5,
        TEXT_PRIMARY,
        data.master_muted,
    );
    hits.push(mute_hit, UiAction::ToggleMasterMute);
    d2d.draw_text(
        &format!("{}%", (data.master_vol * 100.0).round() as i32),
        vol_r.x + vol_r.w - 50.0,
        vol_r.y,
        38.0,
        vol_r.h,
        TEXT_PRIMARY,
        &d2d.font_small,
        2,
    );

    // Brightness Pill
    let br_r = Rect {
        x: bx + half_w + 12.0,
        y: by,
        w: half_w,
        h: 38.0,
    };
    draw_cc_slider(d2d, br_r, data.brightness, AMBER, 2, 0, hits);
    draw_icon_sun(d2d, br_r.x + 21.0, br_r.y + 19.0, 7.5, TEXT_PRIMARY);
    d2d.draw_text(
        &format!("{}%", (data.brightness * 100.0).round() as i32),
        br_r.x + br_r.w - 50.0,
        br_r.y,
        38.0,
        br_r.h,
        TEXT_PRIMARY,
        &d2d.font_small,
        2,
    );

    // Per-App Mixer
    d2d.draw_text(
        "Per-App Audio Mixer",
        bx + 4.0,
        by + 50.0,
        bw,
        18.0,
        TEXT_MUTED,
        &d2d.font_small,
        0,
    );
    for (i, sess) in data.app_sessions.iter().take(4).enumerate() {
        let sy = by + 74.0 + (i as f32) * 32.0;
        d2d.fill_round_rect(bx, sy, bw, 28.0, 9.0, CARD_BG);
        d2d.draw_text(
            &sess.name,
            bx + 12.0,
            sy,
            136.0,
            28.0,
            TEXT_PRIMARY,
            &d2d.font_small,
            0,
        );
        draw_slider(
            d2d,
            Rect {
                x: bx + 154.0,
                y: sy + 8.0,
                w: bw - 208.0,
                h: 12.0,
            },
            sess.volume,
            SKY,
            3,
            sess.pid,
            hits,
        );
        d2d.draw_text(
            &format!("{}%", (sess.volume * 100.0).round() as i32),
            bx + bw - 48.0,
            sy,
            38.0,
            28.0,
            TEXT_SECONDARY,
            &d2d.font_small,
            2,
        );
    }
}

fn draw_tasks_tab(
    d2d: &D2dRenderer,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    hits: &mut UiHitMap,
) {
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
        let active = data.task_filter == flt;
        d2d.fill_round_rect(
            r.x,
            r.y,
            r.w,
            r.h,
            13.0,
            if active { TAB_ACTIVE } else { TRACK_BG },
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
            if active { TEXT_PRIMARY } else { TEXT_MUTED },
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
            w: bw - 42.0,
            h: 32.0,
        },
        data.task_input,
        "Add task (e.g. Call Alex 15:30)...",
        data.focused_input == Some(1),
        1,
        hits,
    );
    let add_r = Rect {
        x: bx + bw - 34.0,
        y: by + 34.0,
        w: 34.0,
        h: 32.0,
    };
    d2d.fill_round_rect(add_r.x, add_r.y, add_r.w, add_r.h, 10.0, PILL_WHITE);
    draw_icon_plus(d2d, add_r.x + 17.0, add_r.y + 16.0, 6.5, ICON_ON_WHITE);
    hits.push(add_r, UiAction::AddTask);

    for (i, t) in data.tasks.iter().take(4).enumerate() {
        let ry = by + 74.0 + (i as f32) * 32.0;
        let row = Rect {
            x: bx,
            y: ry,
            w: bw - 36.0,
            h: 28.0,
        };
        d2d.fill_round_rect(row.x, row.y, row.w, row.h, 9.0, CARD_BG);
        let cbx = row.x + 16.0;
        let cby = row.y + 14.0;
        if t.done {
            d2d.fill_circle(cbx, cby, 7.5, EMERALD);
            draw_icon_check(d2d, cbx, cby, 4.5, ICON_ON_WHITE);
        } else {
            d2d.draw_circle(cbx, cby, 7.5, TEXT_MUTED, 1.3);
        }
        let lbl = if t.due_label.is_empty() {
            t.title.clone()
        } else {
            format!("{}  •  {}", t.title, t.due_label)
        };
        d2d.draw_text(
            &lbl,
            row.x + 32.0,
            row.y,
            row.w - 40.0,
            row.h,
            if t.done { TEXT_MUTED } else { TEXT_PRIMARY },
            &d2d.font_small,
            0,
        );
        hits.push(row, UiAction::ToggleTask(t.id));

        let del = Rect {
            x: bx + bw - 30.0,
            y: ry,
            w: 30.0,
            h: 28.0,
        };
        d2d.fill_round_rect(del.x, del.y, del.w, del.h, 9.0, TRACK_BG);
        draw_icon_trash(d2d, del.x + 15.0, del.y + 14.0, 6.0, ROSE);
        hits.push(del, UiAction::DeleteTask(t.id));
    }
}

fn draw_calendar_tab(d2d: &D2dRenderer, data: &ShellData<'_>, bx: f32, by: f32, bw: f32, bh: f32) {
    d2d.fill_round_rect(bx, by, bw, bh, 14.0, CARD_BG);
    d2d.draw_round_rect(bx, by, bw, bh, 14.0, BORDER_SUBTLE, 1.0);
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
        for (i, ag) in data.agenda.iter().take(5).enumerate() {
            let ay = by + 38.0 + (i as f32) * 30.0;
            d2d.fill_round_rect(bx + 12.0, ay, 56.0, 22.0, 7.0, TRACK_BG);
            d2d.draw_text(
                &ag.time_label,
                bx + 12.0,
                ay,
                56.0,
                22.0,
                SKY,
                &d2d.font_small,
                1,
            );
            d2d.draw_text(
                &ag.title,
                bx + 76.0,
                ay,
                bw - 150.0,
                22.0,
                TEXT_PRIMARY,
                &d2d.font_small,
                0,
            );
            d2d.draw_text(
                &ag.source,
                bx + bw - 68.0,
                ay,
                54.0,
                22.0,
                TEXT_MUTED,
                &d2d.font_small,
                2,
            );
        }
    }
}

fn draw_timer_tab(
    d2d: &D2dRenderer,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    hits: &mut UiHitMap,
) {
    d2d.fill_round_rect(bx, by, bw, bh, 14.0, CARD_BG);
    d2d.draw_round_rect(bx, by, bw, bh, 14.0, BORDER_SUBTLE, 1.0);

    // Left: Large Radial Progress Arc
    let rcx = bx + 84.0;
    let rcy = by + bh * 0.5;
    let rad = 52.0;
    let frac =
        (data.timer.remaining_secs as f32 / (data.timer.total_secs.max(1) as f32)).clamp(0.0, 1.0);
    d2d.draw_circle(rcx, rcy, rad, TRACK_BG, 6.0);
    d2d.draw_arc_ring(
        rcx,
        rcy,
        rad,
        -90.0,
        frac * 360.0,
        if data.timer.running { EMERALD } else { SKY },
        6.0,
    );
    d2d.draw_text(
        &data.timer.formatted_remaining(),
        rcx - 46.0,
        rcy - 18.0,
        92.0,
        36.0,
        TEXT_PRIMARY,
        &d2d.font_bold,
        1,
    );

    // Right: Round dots, Until clock, Controls, Presets
    let rx = bx + 160.0;
    let rw = bw - 176.0;
    d2d.draw_text(
        &data.timer.label,
        rx,
        by + 16.0,
        rw,
        22.0,
        SKY,
        &d2d.font_bold,
        0,
    );
    d2d.draw_text(
        &format!("Until {}", data.timer.until_done_clock()),
        rx,
        by + 38.0,
        rw,
        18.0,
        TEXT_SECONDARY,
        &d2d.font_small,
        0,
    );

    // Round indicator dots
    for i in 0..data.timer.total_rounds.min(6) {
        let filled = i < data.timer.round;
        d2d.fill_circle(
            rx + 6.0 + (i as f32) * 14.0,
            by + 68.0,
            4.0,
            if filled { EMERALD } else { TRACK_BG },
        );
    }

    // Start/Pause + Reset
    let cy = by + 90.0;
    let tog = Rect {
        x: rx,
        y: cy,
        w: 110.0,
        h: 34.0,
    };
    d2d.fill_round_rect(tog.x, tog.y, tog.w, tog.h, 17.0, PILL_WHITE);
    d2d.draw_text(
        if data.timer.running { "Pause" } else { "Start" },
        tog.x,
        tog.y,
        tog.w,
        tog.h,
        ICON_ON_WHITE,
        &d2d.font_bold,
        1,
    );
    hits.push(tog, UiAction::TimerToggle);

    let rst = Rect {
        x: rx + 118.0,
        y: cy,
        w: 84.0,
        h: 34.0,
    };
    d2d.fill_round_rect(rst.x, rst.y, rst.w, rst.h, 17.0, TRACK_BG);
    d2d.draw_text(
        "Reset",
        rst.x,
        rst.y,
        rst.w,
        rst.h,
        TEXT_SECONDARY,
        &d2d.font_small,
        1,
    );
    hits.push(rst, UiAction::TimerReset);

    // Preset pills
    let py = by + 134.0;
    for (i, (lbl, secs)) in [("25m", 1500u64), ("15m", 900u64), ("5m", 300u64)]
        .into_iter()
        .enumerate()
    {
        let pr = Rect {
            x: rx + (i as f32) * 72.0,
            y: py,
            w: 64.0,
            h: 26.0,
        };
        d2d.fill_round_rect(pr.x, pr.y, pr.w, pr.h, 13.0, TRACK_BG);
        d2d.draw_text(
            lbl,
            pr.x,
            pr.y,
            pr.w,
            pr.h,
            TEXT_PRIMARY,
            &d2d.font_small,
            1,
        );
        hits.push(pr, UiAction::TimerPreset(secs));
    }
}

fn draw_apps_tab(
    d2d: &D2dRenderer,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    hits: &mut UiHitMap,
) {
    draw_input(
        d2d,
        Rect {
            x: bx,
            y: by,
            w: bw - 42.0,
            h: 32.0,
        },
        data.app_input,
        "App path or URL (e.g. https://github.com)...",
        data.focused_input == Some(2),
        2,
        hits,
    );
    let pin_b = Rect {
        x: bx + bw - 34.0,
        y: by,
        w: 34.0,
        h: 32.0,
    };
    d2d.fill_round_rect(pin_b.x, pin_b.y, pin_b.w, pin_b.h, 10.0, PILL_WHITE);
    draw_icon_plus(d2d, pin_b.x + 17.0, pin_b.y + 16.0, 6.5, ICON_ON_WHITE);
    hits.push(pin_b, UiAction::AddShortcut);

    // 2-Column Grid of App Shortcut Cards
    let col_w = (bw - 10.0) * 0.5;
    for (i, sc) in data.shortcuts.iter().take(6).enumerate() {
        let col = i % 2;
        let row_i = i / 2;
        let rx = bx + (col as f32) * (col_w + 10.0);
        let ry = by + 42.0 + (row_i as f32) * 46.0;
        let card = Rect {
            x: rx,
            y: ry,
            w: col_w - 28.0,
            h: 38.0,
        };
        d2d.fill_round_rect(card.x, card.y, card.w, card.h, 10.0, CARD_BG);
        let mono_col = album_accent_color(&sc.name);
        d2d.fill_round_rect(card.x + 6.0, card.y + 6.0, 26.0, 26.0, 7.0, mono_col);
        let ch = sc.name.chars().next().unwrap_or('A').to_string();
        d2d.draw_text(
            &ch,
            card.x + 6.0,
            card.y + 6.0,
            26.0,
            26.0,
            ICON_ON_WHITE,
            &d2d.font_bold,
            1,
        );
        d2d.draw_text(
            &sc.name,
            card.x + 38.0,
            card.y + 3.0,
            card.w - 44.0,
            18.0,
            TEXT_PRIMARY,
            &d2d.font_small,
            0,
        );
        d2d.draw_text(
            &sc.target,
            card.x + 38.0,
            card.y + 19.0,
            card.w - 44.0,
            15.0,
            TEXT_MUTED,
            &d2d.font_small,
            0,
        );
        hits.push(card, UiAction::LaunchShortcut(i));

        let del = Rect {
            x: rx + col_w - 24.0,
            y: ry,
            w: 24.0,
            h: 38.0,
        };
        d2d.fill_round_rect(del.x, del.y, del.w, del.h, 8.0, TRACK_BG);
        draw_icon_trash(d2d, del.x + 12.0, del.y + 19.0, 5.5, ROSE);
        hits.push(del, UiAction::RemoveShortcut(sc.id));
    }
}

fn draw_shelf_tab(
    d2d: &D2dRenderer,
    data: &ShellData<'_>,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    hits: &mut UiHitMap,
) {
    let col_w = (bw - 10.0) * 0.5;
    d2d.fill_round_rect(bx, by, col_w, bh, 14.0, CARD_BG);
    d2d.draw_round_rect(bx, by, col_w, bh, 14.0, BORDER_SUBTLE, 1.0);
    d2d.draw_text(
        "File Shelf",
        bx + 12.0,
        by + 8.0,
        col_w - 72.0,
        20.0,
        SKY,
        &d2d.font_bold,
        0,
    );
    let clr = Rect {
        x: bx + col_w - 56.0,
        y: by + 7.0,
        w: 46.0,
        h: 22.0,
    };
    d2d.fill_round_rect(clr.x, clr.y, clr.w, clr.h, 7.0, TRACK_BG);
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

    if data.shelf.is_empty() {
        d2d.draw_text(
            "Drag & drop files onto the notch",
            bx + 12.0,
            by + 44.0,
            col_w - 24.0,
            24.0,
            TEXT_MUTED,
            &d2d.font_small,
            0,
        );
    }
    for (i, it) in data.shelf.iter().take(4).enumerate() {
        let r = Rect {
            x: bx + 8.0,
            y: by + 36.0 + (i as f32) * 32.0,
            w: col_w - 40.0,
            h: 28.0,
        };
        d2d.fill_round_rect(r.x, r.y, r.w, r.h, 8.0, TRACK_BG);
        d2d.draw_text(
            &format!("{} ({})", it.name, it.size_label),
            r.x + 8.0,
            r.y,
            r.w - 14.0,
            r.h,
            TEXT_PRIMARY,
            &d2d.font_small,
            0,
        );
        hits.push(r, UiAction::OpenShelfItem(i));
        let rm = Rect {
            x: bx + col_w - 28.0,
            y: r.y,
            w: 20.0,
            h: 28.0,
        };
        draw_icon_trash(d2d, rm.x + 10.0, rm.y + 14.0, 5.2, ROSE);
        hits.push(rm, UiAction::RemoveShelfItem(i));
    }

    let rx = bx + col_w + 10.0;
    d2d.fill_round_rect(rx, by, col_w, bh, 14.0, CARD_BG);
    d2d.draw_round_rect(rx, by, col_w, bh, 14.0, BORDER_SUBTLE, 1.0);
    d2d.draw_text(
        "Clipboard History",
        rx + 12.0,
        by + 8.0,
        col_w - 24.0,
        20.0,
        EMERALD,
        &d2d.font_bold,
        0,
    );
    for (i, c) in data.clips.iter().take(4).enumerate() {
        let r = Rect {
            x: rx + 8.0,
            y: by + 36.0 + (i as f32) * 32.0,
            w: col_w - 16.0,
            h: 28.0,
        };
        d2d.fill_round_rect(r.x, r.y, r.w, r.h, 8.0, TRACK_BG);
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

fn draw_input(
    d2d: &D2dRenderer,
    r: Rect,
    value: &str,
    placeholder: &str,
    focused: bool,
    input_id: u8,
    hits: &mut UiHitMap,
) {
    d2d.fill_round_rect(r.x, r.y, r.w, r.h, 10.0, TRACK_BG);
    d2d.draw_round_rect(
        r.x,
        r.y,
        r.w,
        r.h,
        10.0,
        if focused { SKY } else { BORDER_SUBTLE },
        if focused { 1.4 } else { 1.0 },
    );
    let txt = if value.is_empty() { placeholder } else { value };
    d2d.draw_text(
        txt,
        r.x + 12.0,
        r.y,
        r.w - 24.0,
        r.h,
        if value.is_empty() {
            TEXT_MUTED
        } else {
            TEXT_PRIMARY
        },
        &d2d.font_small,
        0,
    );
    hits.push(r, UiAction::FocusInput(input_id));
}

fn draw_slider(
    d2d: &D2dRenderer,
    r: Rect,
    val: f32,
    fill: D2D1_COLOR_F,
    kind: u8,
    id: u32,
    hits: &mut UiHitMap,
) {
    let v = val.clamp(0.0, 1.0);
    let cy = r.y + r.h * 0.5;
    d2d.fill_round_rect(r.x, cy - 2.5, r.w, 5.0, 2.5, SURFACE);
    d2d.fill_round_rect(r.x, cy - 2.5, (r.w * v).max(5.0), 5.0, 2.5, fill);
    d2d.fill_circle(r.x + r.w * v, cy, 5.5, PILL_WHITE);
    hits.push_slider(r, kind, id);
}

fn draw_cc_slider(
    d2d: &D2dRenderer,
    r: Rect,
    val: f32,
    fill: D2D1_COLOR_F,
    kind: u8,
    id: u32,
    hits: &mut UiHitMap,
) {
    let v = val.clamp(0.0, 1.0);
    d2d.fill_round_rect(r.x, r.y, r.w, r.h, r.h * 0.5, TRACK_BG);
    let fw = (r.w * v).max(r.h * 0.85);
    d2d.fill_round_rect(
        r.x,
        r.y,
        fw,
        r.h,
        r.h * 0.5,
        rgba(
            (fill.r * 255.0) as u8,
            (fill.g * 255.0) as u8,
            (fill.b * 255.0) as u8,
            0.35,
        ),
    );
    d2d.draw_round_rect(r.x, r.y, r.w, r.h, r.h * 0.5, BORDER_SUBTLE, 1.0);
    hits.push_slider(r, kind, id);
}
