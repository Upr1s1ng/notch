use std::time::Instant;

use crate::config::{AppConfig, NotchStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchState {
    Hidden,
    Collapsed,
    Peek,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedTab {
    Home,
    Media,
    Audio,
    Tasks,
    Calendar,
    Timer,
    Apps,
    Shelf,
}

impl ExpandedTab {
    pub const ALL: [Self; 8] = [
        Self::Home,
        Self::Media,
        Self::Audio,
        Self::Tasks,
        Self::Calendar,
        Self::Timer,
        Self::Apps,
        Self::Shelf,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Media => "Music",
            Self::Audio => "HUD",
            Self::Tasks => "Tasks",
            Self::Calendar => "Cal",
            Self::Timer => "Timer",
            Self::Apps => "Apps",
            Self::Shelf => "Shelf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MascotMood {
    Clock,
    Awake,
    Blink,
    Doze,
    Wave,
}

/// Critically damped spring for snappy transitions that settle cleanly to 0 velocity.
#[derive(Debug, Clone)]
pub struct SpringValue {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
}

impl SpringValue {
    pub fn new(initial: f32) -> Self {
        Self {
            value: initial,
            target: initial,
            velocity: 0.0,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn is_moving(&self) -> bool {
        (self.value - self.target).abs() > 0.25 || self.velocity.abs() > 0.5
    }

    pub fn step(&mut self, dt: f32) -> bool {
        let dt = dt.clamp(0.001, 0.05);
        let stiffness = 340.0_f32;
        let damping = 32.0_f32;
        let accel = stiffness * (self.target - self.value) - damping * self.velocity;
        self.velocity += accel * dt;
        self.value += self.velocity * dt;

        if !self.is_moving() {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct PeekBanner {
    pub title: String,
    pub subtitle: String,
    pub started_at: Instant,
    pub duration_secs: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TickOutcome {
    pub needs_timer: bool,
    pub needs_repaint: bool,
}

pub struct NotchStateMachine {
    pub state: NotchState,
    pub active_tab: ExpandedTab,
    pub width: SpringValue,
    pub height: SpringValue,
    pub top_offset: SpringValue,
    pub corner_radius: SpringValue,
    pub peek: Option<PeekBanner>,
    pub pinned_open: bool,
    pub fullscreen_hidden: bool,
    pub manual_hidden: bool,
    pub input_focused: bool,
    pub mascot_mood: MascotMood,
    pub hover_active: bool,
    hover_since: Option<Instant>,
    leave_since: Option<Instant>,
    last_interaction: Instant,
    last_step: Instant,
}

impl NotchStateMachine {
    pub fn new(cfg: &AppConfig) -> Self {
        let now = Instant::now();
        let (w, h, top, rad) = Self::collapsed_targets(cfg);
        Self {
            state: NotchState::Collapsed,
            active_tab: ExpandedTab::Home,
            width: SpringValue::new(w),
            height: SpringValue::new(h),
            top_offset: SpringValue::new(top),
            corner_radius: SpringValue::new(rad),
            peek: None,
            pinned_open: false,
            fullscreen_hidden: false,
            manual_hidden: false,
            input_focused: false,
            mascot_mood: MascotMood::Clock,
            hover_active: false,
            hover_since: None,
            leave_since: None,
            last_interaction: now,
            last_step: now,
        }
    }

    fn collapsed_targets(cfg: &AppConfig) -> (f32, f32, f32, f32) {
        match cfg.style {
            NotchStyle::TopEdge => (cfg.collapsed_width, cfg.collapsed_height, 0.0, 14.0),
            NotchStyle::FloatingPill => (
                cfg.collapsed_width,
                cfg.collapsed_height,
                cfg.top_margin_px,
                cfg.collapsed_height * 0.5,
            ),
            NotchStyle::Minimal => (cfg.minimal_width, cfg.minimal_height, 0.0, 6.0),
        }
    }

    pub fn apply_targets(&mut self, cfg: &AppConfig) {
        match self.state {
            NotchState::Hidden | NotchState::Collapsed => {
                let (w, h, top, rad) = Self::collapsed_targets(cfg);
                self.width.set_target(w);
                self.height.set_target(h);
                self.top_offset.set_target(top);
                self.corner_radius.set_target(rad);
            }
            NotchState::Peek => {
                let top = if cfg.style == NotchStyle::FloatingPill {
                    cfg.top_margin_px
                } else {
                    0.0
                };
                self.width.set_target(cfg.peek_width);
                self.height.set_target(cfg.peek_height);
                self.top_offset.set_target(top);
                self.corner_radius.set_target(18.0);
            }
            NotchState::Expanded => {
                let top = if cfg.style == NotchStyle::FloatingPill {
                    cfg.top_margin_px
                } else {
                    0.0
                };
                self.width.set_target(cfg.expanded_width);
                self.height.set_target(cfg.expanded_height);
                self.top_offset.set_target(top);
                self.corner_radius.set_target(22.0);
            }
        }
    }

    pub fn collapse(&mut self, cfg: &AppConfig) {
        self.state = if self.fullscreen_hidden || self.manual_hidden {
            NotchState::Hidden
        } else {
            NotchState::Collapsed
        };
        self.peek = None;
        self.hover_since = None;
        self.leave_since = None;
        self.last_interaction = Instant::now();
        self.apply_targets(cfg);
    }

    pub fn expand(&mut self, tab: ExpandedTab, cfg: &AppConfig) {
        if self.fullscreen_hidden || self.manual_hidden {
            return;
        }
        self.state = NotchState::Expanded;
        self.active_tab = tab;
        self.peek = None;
        self.hover_since = None;
        self.leave_since = None;
        self.last_interaction = Instant::now();
        self.mascot_mood = MascotMood::Wave;
        self.apply_targets(cfg);
    }

    pub fn toggle_expand(&mut self, cfg: &AppConfig) {
        if self.state == NotchState::Expanded {
            self.pinned_open = false;
            self.collapse(cfg);
        } else if self.state != NotchState::Hidden {
            self.expand(self.active_tab, cfg);
        }
    }

    pub fn trigger_peek(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        cfg: &AppConfig,
    ) {
        if self.fullscreen_hidden || self.manual_hidden || self.state == NotchState::Expanded {
            return;
        }
        self.state = NotchState::Peek;
        self.peek = Some(PeekBanner {
            title: title.into(),
            subtitle: subtitle.into(),
            started_at: Instant::now(),
            duration_secs: (cfg.peek_duration_ms.max(500) as f32) / 1000.0,
        });
        self.last_interaction = Instant::now();
        self.apply_targets(cfg);
    }

    pub fn toggle_manual_hide(&mut self, cfg: &AppConfig) {
        self.manual_hidden = !self.manual_hidden;
        self.state = if self.manual_hidden || self.fullscreen_hidden {
            NotchState::Hidden
        } else {
            NotchState::Collapsed
        };
        self.apply_targets(cfg);
    }

    pub fn set_fullscreen_hidden(&mut self, hidden: bool, cfg: &AppConfig) {
        if self.fullscreen_hidden == hidden {
            return;
        }
        self.fullscreen_hidden = hidden;
        self.state = if hidden || self.manual_hidden {
            NotchState::Hidden
        } else {
            NotchState::Collapsed
        };
        self.peek = None;
        self.apply_targets(cfg);
    }

    pub fn on_hover_change(&mut self, hovering: bool, _cfg: &AppConfig) {
        self.hover_active = hovering;
        let now = Instant::now();
        if hovering {
            self.leave_since = None;
            if self.state == NotchState::Collapsed && self.hover_since.is_none() {
                self.hover_since = Some(now);
            }
            if self.mascot_mood == MascotMood::Doze {
                self.mascot_mood = MascotMood::Wave;
            }
        } else {
            self.hover_since = None;
            if self.state == NotchState::Expanded
                && !self.input_focused
                && !self.pinned_open
                && self.leave_since.is_none()
            {
                self.leave_since = Some(now);
            }
        }
        self.last_interaction = now;
    }

    pub fn update_mascot_idle(&mut self, cfg: &AppConfig, has_live_activity: bool) {
        if !cfg.mascot_enabled || has_live_activity || self.state != NotchState::Collapsed {
            self.mascot_mood = MascotMood::Clock;
            return;
        }
        let idle = self.last_interaction.elapsed().as_secs();
        if idle >= cfg.mascot_idle_secs + 12 {
            self.mascot_mood = MascotMood::Doze;
        } else if idle >= cfg.mascot_idle_secs {
            self.mascot_mood = MascotMood::Awake;
        } else {
            self.mascot_mood = MascotMood::Clock;
        }
    }

    pub fn tick(&mut self, cfg: &AppConfig) -> TickOutcome {
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_step)
            .as_secs_f32()
            .clamp(0.001, 0.05);
        self.last_step = now;
        let mut changed = false;

        if self.state == NotchState::Peek {
            if let Some(p) = &self.peek {
                if now.duration_since(p.started_at).as_secs_f32() >= p.duration_secs {
                    self.collapse(cfg);
                    changed = true;
                }
            }
        }

        if self.state == NotchState::Collapsed && self.hover_active {
            if let Some(since) = self.hover_since {
                if now.duration_since(since).as_millis() as u64 >= cfg.hover_expand_delay_ms {
                    self.expand(self.active_tab, cfg);
                    changed = true;
                }
            }
        } else if self.state == NotchState::Expanded
            && !self.hover_active
            && !self.input_focused
            && !self.pinned_open
        {
            if let Some(since) = self.leave_since {
                if now.duration_since(since).as_millis() as u64 >= cfg.hover_collapse_delay_ms {
                    self.collapse(cfg);
                    changed = true;
                }
            }
        }

        let mw = self.width.step(dt);
        let mh = self.height.step(dt);
        let mt = self.top_offset.step(dt);
        let mr = self.corner_radius.step(dt);
        let anim = mw || mh || mt || mr;
        let pending = self.hover_since.is_some()
            || self.leave_since.is_some()
            || self.state == NotchState::Peek;

        TickOutcome {
            needs_timer: anim || pending,
            needs_repaint: anim || changed || self.state == NotchState::Peek,
        }
    }
}
