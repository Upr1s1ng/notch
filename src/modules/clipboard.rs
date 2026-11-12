use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

const CF_UNICODETEXT: u32 = 13;

#[derive(Debug, Clone)]
pub struct ClipEntry {
    pub text: String,
    pub preview: String,
}

#[derive(Default)]
pub struct ClipboardHistory {
    pub entries: Vec<ClipEntry>,
}

impl ClipboardHistory {
    pub fn capture_if_safe(&mut self, owner: HWND) {
        if is_sensitive_source() {
            return;
        }
        unsafe {
            if !IsClipboardFormatAvailable(CF_UNICODETEXT).is_ok() {
                return;
            }
            let fmt_exclude =
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            let fmt_ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
            let fmt_history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));

            if (fmt_exclude != 0 && IsClipboardFormatAvailable(fmt_exclude).is_ok())
                || (fmt_ignore != 0 && IsClipboardFormatAvailable(fmt_ignore).is_ok())
            {
                return;
            }

            if OpenClipboard(Some(owner)).is_err() {
                return;
            }

            // Check CanIncludeInClipboardHistory == 0
            if fmt_history != 0 && IsClipboardFormatAvailable(fmt_history).is_ok() {
                if let Ok(h) = GetClipboardData(fmt_history) {
                    let hmem = windows::Win32::Foundation::HGLOBAL(h.0);
                    let ptr = GlobalLock(hmem) as *const u32;
                    let allow = if !ptr.is_null() { *ptr != 0 } else { true };
                    let _ = GlobalUnlock(hmem);
                    if !allow {
                        let _ = CloseClipboard();
                        return;
                    }
                }
            }

            let mut captured = None;
            if let Ok(h) = GetClipboardData(CF_UNICODETEXT) {
                let hmem = windows::Win32::Foundation::HGLOBAL(h.0);
                let sz = GlobalSize(hmem);
                let ptr = GlobalLock(hmem) as *const u16;
                if !ptr.is_null() && sz >= 2 {
                    let slice = std::slice::from_raw_parts(ptr, sz / 2);
                    let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
                    let txt = String::from_utf16_lossy(&slice[..end]);
                    if !txt.trim().is_empty() {
                        captured = Some(txt);
                    }
                }
                let _ = GlobalUnlock(hmem);
            }
            let _ = CloseClipboard();

            if let Some(text) = captured {
                self.entries.retain(|e| e.text != text);
                let preview: String = text.trim().replace('\n', " ").chars().take(46).collect();
                self.entries.insert(0, ClipEntry { text, preview });
                if self.entries.len() > 16 {
                    self.entries.truncate(16);
                }
            }
        }
    }

    pub fn copy_to_clipboard(&self, owner: HWND, text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * 2;
        unsafe {
            let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
                return;
            };
            let ptr = GlobalLock(hmem) as *mut u16;
            if ptr.is_null() {
                return;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            let _ = GlobalUnlock(hmem);
            if OpenClipboard(Some(owner)).is_ok() {
                let _ = EmptyClipboard();
                let _ = SetClipboardData(
                    CF_UNICODETEXT,
                    Some(windows::Win32::Foundation::HANDLE(hmem.0)),
                );
                let _ = CloseClipboard();
            }
        }
    }
}

fn is_sensitive_source() -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() {
            return false;
        }
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(fg, &mut buf) as usize;
        let title = String::from_utf16_lossy(&buf[..len]).to_ascii_lowercase();
        title.contains("keepass") || title.contains("bitwarden") || title.contains("1password")
    }
}
