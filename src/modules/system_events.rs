use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, GetKeyboardLayout, VK_CAPITAL};

pub struct SystemEventMonitor {
    last_caps: bool,
    last_klid: usize,
    last_ac_line: u8,
    last_low_bat_warned: bool,
    last_vol_pct: i32,
}

impl SystemEventMonitor {
    pub fn new(initial_vol: f32) -> Self {
        let (caps, klid, ac, _pct) = Self::sample();
        Self {
            last_caps: caps,
            last_klid: klid,
            last_ac_line: ac,
            last_low_bat_warned: false,
            last_vol_pct: (initial_vol * 100.0).round() as i32,
        }
    }

    fn sample() -> (bool, usize, u8, u8) {
        unsafe {
            let caps = (GetKeyState(VK_CAPITAL.0 as i32) & 1) != 0;
            let klid = GetKeyboardLayout(0).0 as usize;
            let mut sps = SYSTEM_POWER_STATUS::default();
            let (ac, pct) = if GetSystemPowerStatus(&mut sps).is_ok() {
                (sps.ACLineStatus, sps.BatteryLifePercent)
            } else {
                (255, 255)
            };
            (caps, klid, ac, pct)
        }
    }

    /// Polls lightweight Win32 state and returns an optional transient Peek (title, subtitle).
    pub fn poll(&mut self, current_vol: f32) -> Option<(String, String)> {
        let (caps, klid, ac, pct) = Self::sample();
        if caps != self.last_caps {
            self.last_caps = caps;
            return Some((
                if caps {
                    "Caps Lock ON".into()
                } else {
                    "Caps Lock OFF".into()
                },
                "Keyboard state updated".into(),
            ));
        }
        if klid != self.last_klid {
            self.last_klid = klid;
            return Some((
                "Keyboard Language".into(),
                format!("Layout ID 0x{:04X}", klid & 0xFFFF),
            ));
        }
        if ac != self.last_ac_line && ac != 255 {
            self.last_ac_line = ac;
            if ac == 1 {
                return Some((
                    "Charger Connected".into(),
                    format!("Battery {pct}% • AC Power"),
                ));
            }
        }
        if pct <= 15 && ac == 0 && !self.last_low_bat_warned {
            self.last_low_bat_warned = true;
            return Some(("Low Battery".into(), format!("{pct}% remaining")));
        } else if pct > 20 || ac == 1 {
            self.last_low_bat_warned = false;
        }
        let v_pct = (current_vol * 100.0).round() as i32;
        if (v_pct - self.last_vol_pct).abs() >= 2 {
            self.last_vol_pct = v_pct;
            return Some(("Volume".into(), format!("Master output {v_pct}%")));
        }
        None
    }
}
