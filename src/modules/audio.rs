use windows::core::Interface;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

#[derive(Debug, Clone)]
pub struct AppAudioSession {
    pub pid: u32,
    pub name: String,
    pub volume: f32,
    pub muted: bool,
}

pub struct AudioController;

impl AudioController {
    pub fn new() -> Self {
        Self
    }

    fn endpoint_volume(&self) -> Option<IAudioEndpointVolume> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
            let dev = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            dev.Activate(CLSCTX_ALL, None).ok()
        }
    }

    fn session_manager(&self) -> Option<IAudioSessionManager2> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
            let dev = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            dev.Activate(CLSCTX_ALL, None).ok()
        }
    }

    pub fn get_master_volume(&self) -> (f32, bool) {
        let Some(ep) = self.endpoint_volume() else {
            return (0.5, false);
        };
        unsafe {
            let vol = ep.GetMasterVolumeLevelScalar().unwrap_or(0.5);
            let muted = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
            (vol.clamp(0.0, 1.0), muted)
        }
    }

    pub fn set_master_volume(&self, ratio: f32) {
        if let Some(ep) = self.endpoint_volume() {
            unsafe {
                let _ = ep.SetMasterVolumeLevelScalar(ratio.clamp(0.0, 1.0), std::ptr::null());
            }
        }
    }

    pub fn toggle_master_mute(&self) -> bool {
        let Some(ep) = self.endpoint_volume() else {
            return false;
        };
        unsafe {
            let muted = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
            let next = !muted;
            let _ = ep.SetMute(next, std::ptr::null());
            next
        }
    }

    pub fn list_app_sessions(&self) -> Vec<AppAudioSession> {
        let Some(mgr) = self.session_manager() else {
            return Vec::new();
        };
        let Ok(enumerator) = (unsafe { mgr.GetSessionEnumerator() }) else {
            return Vec::new();
        };
        let count = unsafe { enumerator.GetCount().unwrap_or(0) };
        let proc_names = snapshot_process_names();
        let mut out = Vec::new();
        for i in 0..count {
            unsafe {
                let Ok(ctrl) = enumerator.GetSession(i) else {
                    continue;
                };
                let Ok(ctrl2): Result<IAudioSessionControl2, _> = ctrl.cast() else {
                    continue;
                };
                let pid = ctrl2.GetProcessId().unwrap_or(0);
                if pid == 0 {
                    continue;
                }
                let Ok(simple): Result<ISimpleAudioVolume, _> = ctrl.cast() else {
                    continue;
                };
                let vol = simple.GetMasterVolume().unwrap_or(1.0);
                let muted = simple.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                let name = proc_names
                    .iter()
                    .find(|(p, _)| *p == pid)
                    .map(|(_, n)| n.clone())
                    .unwrap_or_else(|| format!("PID {pid}"));
                out.push(AppAudioSession {
                    pid,
                    name,
                    volume: vol.clamp(0.0, 1.0),
                    muted,
                });
            }
        }
        out
    }

    pub fn set_app_volume(&self, target_pid: u32, ratio: f32) {
        let Some(mgr) = self.session_manager() else {
            return;
        };
        let Ok(enumerator) = (unsafe { mgr.GetSessionEnumerator() }) else {
            return;
        };
        let count = unsafe { enumerator.GetCount().unwrap_or(0) };
        for i in 0..count {
            unsafe {
                if let Ok(ctrl) = enumerator.GetSession(i) {
                    if let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() {
                        if ctrl2.GetProcessId().unwrap_or(0) == target_pid {
                            if let Ok(simple) = ctrl.cast::<ISimpleAudioVolume>() {
                                let _ =
                                    simple.SetMasterVolume(ratio.clamp(0.0, 1.0), std::ptr::null());
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn toggle_exe_mute(&self, exe_name: &str) -> Option<bool> {
        let sessions = self.list_app_sessions();
        let found = sessions
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case(exe_name))?;
        let next = !found.muted;
        let Some(mgr) = self.session_manager() else {
            return None;
        };
        let Ok(enumerator) = (unsafe { mgr.GetSessionEnumerator() }) else {
            return None;
        };
        let count = unsafe { enumerator.GetCount().unwrap_or(0) };
        for i in 0..count {
            unsafe {
                if let Ok(ctrl) = enumerator.GetSession(i) {
                    if let Ok(ctrl2) = ctrl.cast::<IAudioSessionControl2>() {
                        if ctrl2.GetProcessId().unwrap_or(0) == found.pid {
                            if let Ok(simple) = ctrl.cast::<ISimpleAudioVolume>() {
                                let _ = simple.SetMute(next, std::ptr::null());
                                return Some(next);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

fn snapshot_process_names() -> Vec<(u32, String)> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return out;
        };
        let mut pe = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                let end = pe
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(pe.szExeFile.len());
                let name = String::from_utf16_lossy(&pe.szExeFile[..end]);
                out.push((pe.th32ProcessID, name));
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
    }
    out
}
