use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::json_mini::{self, JsonVal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchStyle {
    TopEdge,
    FloatingPill,
    Minimal,
}

impl NotchStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TopEdge => "top_edge",
            Self::FloatingPill => "floating_pill",
            Self::Minimal => "minimal",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "floating_pill" => Self::FloatingPill,
            "minimal" => Self::Minimal,
            _ => Self::TopEdge,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    Fixed,
    Smart,
    Peek,
}

impl VisibilityMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Smart => "smart",
            Self::Peek => "peek",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "smart" => Self::Smart,
            "peek" => Self::Peek,
            _ => Self::Fixed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub style: NotchStyle,
    pub visibility_mode: VisibilityMode,
    pub mascot_enabled: bool,
    pub exclude_from_capture: bool,
    pub hide_in_fullscreen: bool,
    pub dual_hwnd_glass: bool,
    pub collapsed_width: f32,
    pub collapsed_height: f32,
    pub minimal_width: f32,
    pub minimal_height: f32,
    pub peek_width: f32,
    pub peek_height: f32,
    pub expanded_width: f32,
    pub expanded_height: f32,
    pub top_margin_px: f32,
    pub hover_expand_delay_ms: u64,
    pub hover_collapse_delay_ms: u64,
    pub peek_duration_ms: u64,
    pub mascot_idle_secs: u64,
    pub discord_mute_hotkey: String,
    pub discord_deafen_hotkey: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            style: NotchStyle::TopEdge,
            visibility_mode: VisibilityMode::Fixed,
            mascot_enabled: true,
            exclude_from_capture: false,
            hide_in_fullscreen: true,
            dual_hwnd_glass: true,
            collapsed_width: 214.0,
            collapsed_height: 32.0,
            minimal_width: 148.0,
            minimal_height: 10.0,
            peek_width: 376.0,
            peek_height: 62.0,
            expanded_width: 560.0,
            expanded_height: 304.0,
            top_margin_px: 6.0,
            hover_expand_delay_ms: 240,
            hover_collapse_delay_ms: 360,
            peek_duration_ms: 2500,
            mascot_idle_secs: 18,
            discord_mute_hotkey: "Ctrl+Shift+M".into(),
            discord_deafen_hotkey: "Ctrl+Shift+D".into(),
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("Notch");
    let _ = fs::create_dir_all(&dir);
    dir
}

impl AppConfig {
    pub fn load() -> Self {
        let path = app_data_dir().join("config.json");
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(JsonVal::Obj(map)) = json_mini::parse(&raw) else {
            return Self::default();
        };
        let mut cfg = Self::default();
        if let Some(s) = map.get("style").and_then(JsonVal::as_str) {
            cfg.style = NotchStyle::from_str(s);
        }
        if let Some(s) = map.get("visibility_mode").and_then(JsonVal::as_str) {
            cfg.visibility_mode = VisibilityMode::from_str(s);
        }
        if let Some(b) = map.get("mascot_enabled").and_then(JsonVal::as_bool) {
            cfg.mascot_enabled = b;
        }
        if let Some(b) = map.get("exclude_from_capture").and_then(JsonVal::as_bool) {
            cfg.exclude_from_capture = b;
        }
        if let Some(b) = map.get("hide_in_fullscreen").and_then(JsonVal::as_bool) {
            cfg.hide_in_fullscreen = b;
        }
        if let Some(b) = map.get("dual_hwnd_glass").and_then(JsonVal::as_bool) {
            cfg.dual_hwnd_glass = b;
        }
        if let Some(n) = map.get("peek_duration_ms").and_then(JsonVal::as_f64) {
            cfg.peek_duration_ms = n.clamp(800.0, 10000.0) as u64;
        }
        if let Some(s) = map.get("discord_mute_hotkey").and_then(JsonVal::as_str) {
            cfg.discord_mute_hotkey = s.to_string();
        }
        if let Some(s) = map.get("discord_deafen_hotkey").and_then(JsonVal::as_str) {
            cfg.discord_deafen_hotkey = s.to_string();
        }
        cfg
    }

    pub fn save(&self) {
        let mut map = BTreeMap::new();
        map.insert("style".into(), JsonVal::Str(self.style.as_str().into()));
        map.insert(
            "visibility_mode".into(),
            JsonVal::Str(self.visibility_mode.as_str().into()),
        );
        map.insert("mascot_enabled".into(), JsonVal::Bool(self.mascot_enabled));
        map.insert(
            "exclude_from_capture".into(),
            JsonVal::Bool(self.exclude_from_capture),
        );
        map.insert(
            "hide_in_fullscreen".into(),
            JsonVal::Bool(self.hide_in_fullscreen),
        );
        map.insert(
            "dual_hwnd_glass".into(),
            JsonVal::Bool(self.dual_hwnd_glass),
        );
        map.insert(
            "peek_duration_ms".into(),
            JsonVal::Num(self.peek_duration_ms as f64),
        );
        map.insert(
            "discord_mute_hotkey".into(),
            JsonVal::Str(self.discord_mute_hotkey.clone()),
        );
        map.insert(
            "discord_deafen_hotkey".into(),
            JsonVal::Str(self.discord_deafen_hotkey.clone()),
        );
        let path = app_data_dir().join("config.json");
        let _ = fs::write(path, json_mini::to_string_pretty(&JsonVal::Obj(map)));
    }
}
