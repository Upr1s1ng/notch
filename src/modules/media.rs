use std::time::Instant;

use windows::Foundation::TimeSpan;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_KEYUP, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
};

#[derive(Debug, Clone)]
pub struct MediaSnapshot {
    pub has_session: bool,
    pub source_app_id: String,
    pub available_sources: Vec<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub captured_at: Instant,
}

impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            has_session: false,
            source_app_id: String::new(),
            available_sources: Vec::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            is_playing: false,
            position_secs: 0.0,
            duration_secs: 0.0,
            captured_at: Instant::now(),
        }
    }
}

impl MediaSnapshot {
    pub fn live_position_secs(&self) -> f64 {
        if !self.is_playing {
            return self.position_secs;
        }
        let elapsed = self.captured_at.elapsed().as_secs_f64();
        let pos = (self.position_secs + elapsed).max(0.0);
        if self.duration_secs > 0.0 {
            pos.min(self.duration_secs)
        } else {
            pos
        }
    }

    pub fn track_key(&self) -> String {
        format!("{}|{}|{}", self.title, self.artist, self.album)
    }
}

pub struct SmtcController {
    selected_idx: usize,
}

impl SmtcController {
    pub fn new() -> Self {
        Self { selected_idx: 0 }
    }

    fn sessions(&self) -> Vec<GlobalSystemMediaTransportControlsSession> {
        let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() else {
            return Vec::new();
        };
        let Ok(mgr) = op.get() else {
            return Vec::new();
        };
        let Ok(list) = mgr.GetSessions() else {
            return Vec::new();
        };
        let count = list.Size().unwrap_or(0);
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            if let Ok(s) = list.GetAt(i) {
                out.push(s);
            }
        }
        out
    }

    fn active_session(&self) -> Option<(GlobalSystemMediaTransportControlsSession, Vec<String>)> {
        let list = self.sessions();
        if list.is_empty() {
            return None;
        }
        let sources: Vec<String> = list
            .iter()
            .map(|s| {
                s.SourceAppUserModelId()
                    .map(|h| h.to_string())
                    .unwrap_or_default()
            })
            .collect();
        let idx = self.selected_idx % list.len();
        Some((list[idx].clone(), sources))
    }

    pub fn cycle_source(&mut self) {
        self.selected_idx = self.selected_idx.wrapping_add(1);
    }

    pub fn read_snapshot(&self) -> MediaSnapshot {
        let Some((session, sources)) = self.active_session() else {
            return MediaSnapshot::default();
        };
        let mut snap = MediaSnapshot {
            has_session: true,
            source_app_id: session
                .SourceAppUserModelId()
                .map(|h| h.to_string())
                .unwrap_or_default(),
            available_sources: sources,
            captured_at: Instant::now(),
            ..Default::default()
        };

        if let Ok(op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(props) = op.get() {
                snap.title = props.Title().map(|h| h.to_string()).unwrap_or_default();
                snap.artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
                snap.album = props
                    .AlbumTitle()
                    .map(|h| h.to_string())
                    .unwrap_or_default();
            }
        }
        if let Ok(info) = session.GetPlaybackInfo() {
            if let Ok(status) = info.PlaybackStatus() {
                snap.is_playing =
                    status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
            }
        }
        if let Ok(tl) = session.GetTimelineProperties() {
            if let Ok(pos) = tl.Position() {
                snap.position_secs = (pos.Duration as f64) / 10_000_000.0;
            }
            if let Ok(end) = tl.EndTime() {
                snap.duration_secs = (end.Duration as f64) / 10_000_000.0;
            }
        }
        snap
    }

    pub fn toggle_play_pause(&self) {
        if let Some((s, _)) = self.active_session() {
            if let Ok(op) = s.TryTogglePlayPauseAsync() {
                let _ = op.get();
                return;
            }
        }
        tap_vk(VK_MEDIA_PLAY_PAUSE.0 as u8);
    }

    pub fn next_track(&self) {
        if let Some((s, _)) = self.active_session() {
            if let Ok(op) = s.TrySkipNextAsync() {
                let _ = op.get();
                return;
            }
        }
        tap_vk(VK_MEDIA_NEXT_TRACK.0 as u8);
    }

    pub fn prev_track(&self) {
        if let Some((s, _)) = self.active_session() {
            if let Ok(op) = s.TrySkipPreviousAsync() {
                let _ = op.get();
                return;
            }
        }
        tap_vk(VK_MEDIA_PREV_TRACK.0 as u8);
    }

    pub fn seek_to_seconds(&self, secs: f64) {
        if let Some((s, _)) = self.active_session() {
            let ticks = (secs.max(0.0) * 10_000_000.0) as i64;
            if let Ok(op) = s.TryChangePlaybackPositionAsync(ticks) {
                let _ = op.get();
            }
        }
        let _ = TimeSpan { Duration: 0 };
    }
}

fn tap_vk(vk: u8) {
    unsafe {
        keybd_event(vk, 0, Default::default(), 0);
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
    }
}
