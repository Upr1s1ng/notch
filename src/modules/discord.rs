use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    VK_CONTROL, VK_LWIN, VK_MENU, VK_SHIFT,
};

use crate::config::AppConfig;
use crate::json_mini::{self, JsonVal};
use crate::modules::audio::AudioController;

#[derive(Debug, Clone, Default)]
pub struct DiscordVoiceState {
    pub discord_running: bool,
    pub ipc_connected: bool,
    pub in_call: bool,
    pub channel_name: String,
    pub muted: bool,
    pub deafened: bool,
}

pub fn is_discord_running() -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return false;
        };
        let mut pe = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut found = false;
        if Process32FirstW(snap, &mut pe).is_ok() {
            loop {
                let end = pe
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(pe.szExeFile.len());
                let name = String::from_utf16_lossy(&pe.szExeFile[..end]);
                if name.eq_ignore_ascii_case("Discord.exe")
                    || name.eq_ignore_ascii_case("DiscordPTB.exe")
                    || name.eq_ignore_ascii_case("DiscordCanary.exe")
                {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut pe).is_err() {
                    break;
                }
            }
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
        found
    }
}

pub fn send_hotkey_combo(combo: &str) -> bool {
    let parts: Vec<&str> = combo
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return false;
    }
    let mut vks = Vec::new();
    for p in parts {
        let vk = match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => VK_CONTROL,
            "shift" => VK_SHIFT,
            "alt" => VK_MENU,
            "win" => VK_LWIN,
            s if s.len() == 1 => VIRTUAL_KEY(s.as_bytes()[0].to_ascii_uppercase() as u16),
            _ => continue,
        };
        vks.push(vk);
    }
    if vks.is_empty() {
        return false;
    }
    let mut inputs = Vec::with_capacity(vks.len() * 2);
    for &vk in &vks {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    ..Default::default()
                },
            },
        });
    }
    for &vk in vks.iter().rev() {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        });
    }
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) > 0 }
}

pub struct DiscordController {
    pub state: Arc<Mutex<DiscordVoiceState>>,
    stop_flag: Arc<AtomicBool>,
    cfg: AppConfig,
}

impl DiscordController {
    pub fn new(cfg: AppConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(DiscordVoiceState::default())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            cfg,
        }
    }

    pub fn start(&self) {
        let st = self.state.clone();
        let stop = self.stop_flag.clone();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let running = is_discord_running();
                if let Ok(mut guard) = st.lock() {
                    guard.discord_running = running;
                }
                if running {
                    // Try Discord Local IPC pipe; if unavailable, stay in fallback mode cleanly
                    if let Ok(mut pipe) = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(r"\\.\pipe\discord-ipc-0")
                    {
                        let handshake = r#"{"v":1,"client_id":"1172264702067163186"}"#;
                        let _ = write_frame(&mut pipe, 0, handshake.as_bytes());
                        if let Ok((_op, payload)) = read_frame(&mut pipe) {
                            if let Ok(s) = std::str::from_utf8(&payload) {
                                if let Ok(JsonVal::Obj(map)) = json_mini::parse(s) {
                                    let ready =
                                        map.get("evt").and_then(JsonVal::as_str) == Some("READY");
                                    if let Ok(mut guard) = st.lock() {
                                        guard.ipc_connected = ready;
                                    }
                                }
                            }
                        }
                    } else if let Ok(mut guard) = st.lock() {
                        guard.ipc_connected = false;
                    }
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        });
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> DiscordVoiceState {
        self.state.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn toggle_mute(&self, audio: &AudioController) {
        let mut next = false;
        if let Ok(mut guard) = self.state.lock() {
            guard.muted = !guard.muted;
            next = guard.muted;
        }
        if !self.cfg.discord_mute_hotkey.is_empty() {
            send_hotkey_combo(&self.cfg.discord_mute_hotkey);
        } else {
            let _ = audio.toggle_exe_mute("Discord.exe");
        }
        let _ = next;
    }

    pub fn toggle_deafen(&self, audio: &AudioController) {
        if let Ok(mut guard) = self.state.lock() {
            guard.deafened = !guard.deafened;
            if guard.deafened {
                guard.muted = true;
            }
        }
        if !self.cfg.discord_deafen_hotkey.is_empty() {
            send_hotkey_combo(&self.cfg.discord_deafen_hotkey);
        } else {
            let _ = audio.toggle_exe_mute("Discord.exe");
        }
    }

    pub fn disconnect_call(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.in_call = false;
            guard.channel_name.clear();
        }
    }
}

fn write_frame(pipe: &mut std::fs::File, opcode: u32, payload: &[u8]) -> std::io::Result<()> {
    let mut hdr = [0u8; 8];
    hdr[..4].copy_from_slice(&opcode.to_le_bytes());
    hdr[4..].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    pipe.write_all(&hdr)?;
    pipe.write_all(payload)?;
    pipe.flush()
}

fn read_frame(pipe: &mut std::fs::File) -> std::io::Result<(u32, Vec<u8>)> {
    let mut hdr = [0u8; 8];
    pipe.read_exact(&mut hdr)?;
    let op = u32::from_le_bytes(hdr[..4].try_into().unwrap());
    let len = u32::from_le_bytes(hdr[4..].try_into().unwrap()).min(65536) as usize;
    let mut buf = vec![0u8; len];
    pipe.read_exact(&mut buf)?;
    Ok((op, buf))
}
