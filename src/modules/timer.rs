use std::time::Instant;

use windows::Win32::System::SystemInformation::GetLocalTime;

pub struct TimerState {
    pub label: String,
    pub total_secs: u64,
    pub remaining_secs: f64,
    pub running: bool,
    pub round: u32,
    pub total_rounds: u32,
    last_tick: Instant,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            label: "Pomodoro Focus".into(),
            total_secs: 25 * 60,
            remaining_secs: (25 * 60) as f64,
            running: false,
            round: 1,
            total_rounds: 4,
            last_tick: Instant::now(),
        }
    }
}

impl TimerState {
    pub fn set_preset(&mut self, secs: u64, label: &str) {
        self.total_secs = secs.max(1);
        self.remaining_secs = self.total_secs as f64;
        self.label = label.to_string();
        self.running = false;
        self.last_tick = Instant::now();
    }

    pub fn toggle(&mut self) {
        self.running = !self.running;
        self.last_tick = Instant::now();
    }

    pub fn reset(&mut self) {
        self.remaining_secs = self.total_secs as f64;
        self.running = false;
        self.last_tick = Instant::now();
    }

    /// Returns true if the timer just finished this tick.
    pub fn tick(&mut self) -> bool {
        if !self.running {
            return false;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        self.remaining_secs = (self.remaining_secs - dt).max(0.0);
        if self.remaining_secs <= 0.0 {
            self.running = false;
            self.round = if self.round >= self.total_rounds {
                1
            } else {
                self.round + 1
            };
            return true;
        }
        false
    }

    pub fn formatted_remaining(&self) -> String {
        let s = self.remaining_secs.ceil() as u64;
        format!("{:02}:{:02}", s / 60, s % 60)
    }

    pub fn until_done_clock(&self) -> String {
        unsafe {
            let lt = GetLocalTime();
            let now_mins = (lt.wHour as u64) * 60 + (lt.wMinute as u64);
            let add_mins = (self.remaining_secs.ceil() as u64 + 59) / 60;
            let done_mins = (now_mins + add_mins) % (24 * 60);
            format!("{:02}:{:02}", done_mins / 60, done_mins % 60)
        }
    }

    pub fn progress(&self) -> f32 {
        let total = self.total_secs.max(1) as f64;
        (1.0 - (self.remaining_secs / total)).clamp(0.0, 1.0) as f32
    }
}
