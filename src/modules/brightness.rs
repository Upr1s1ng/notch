use windows::Win32::Devices::Display::{
    DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{
    MonitorFromPoint, MonitorFromWindow, MONITOR_DEFAULTTONEAREST,
};

#[derive(Debug, Clone, Copy)]
pub struct BrightnessState {
    pub level: f32,
    pub supported: bool,
}

pub struct BrightnessController;

impl BrightnessController {
    pub fn new() -> Self {
        Self
    }

    pub fn get_brightness(&self, hwnd: Option<HWND>) -> BrightnessState {
        unsafe {
            let hmon = match hwnd {
                Some(h) if !h.is_invalid() => MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST),
                _ => MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST),
            };
            let mut count: u32 = 0;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_err() || count == 0 {
                return BrightnessState {
                    level: 0.75,
                    supported: false,
                };
            }
            let mut mons = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hmon, &mut mons).is_err() {
                return BrightnessState {
                    level: 0.75,
                    supported: false,
                };
            }
            let mut min_b = 0u32;
            let mut cur_b = 75u32;
            let mut max_b = 100u32;
            let ok =
                GetMonitorBrightness(mons[0].hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b)
                    != 0;
            let _ = DestroyPhysicalMonitors(&mons);
            if ok && max_b > min_b {
                let ratio = ((cur_b.saturating_sub(min_b)) as f32) / ((max_b - min_b) as f32);
                BrightnessState {
                    level: ratio.clamp(0.0, 1.0),
                    supported: true,
                }
            } else {
                BrightnessState {
                    level: 0.75,
                    supported: false,
                }
            }
        }
    }

    pub fn set_brightness(&self, ratio: f32, hwnd: Option<HWND>) -> bool {
        unsafe {
            let hmon = match hwnd {
                Some(h) if !h.is_invalid() => MonitorFromWindow(h, MONITOR_DEFAULTTONEAREST),
                _ => MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST),
            };
            let mut count: u32 = 0;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(hmon, &mut count).is_err() || count == 0 {
                return false;
            }
            let mut mons = vec![PHYSICAL_MONITOR::default(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(hmon, &mut mons).is_err() {
                return false;
            }
            let mut min_b = 0u32;
            let mut cur_b = 75u32;
            let mut max_b = 100u32;
            let mut ok = false;
            if GetMonitorBrightness(mons[0].hPhysicalMonitor, &mut min_b, &mut cur_b, &mut max_b)
                != 0
                && max_b > min_b
            {
                let target =
                    min_b + ((ratio.clamp(0.0, 1.0) * (max_b - min_b) as f32).round() as u32);
                ok = SetMonitorBrightness(mons[0].hPhysicalMonitor, target) != 0;
            }
            let _ = DestroyPhysicalMonitors(&mons);
            ok
        }
    }
}
