use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub fn build_google_search_url(query: &str) -> String {
    let mut enc = String::with_capacity(query.len());
    for b in query.trim().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                enc.push(*b as char)
            }
            b' ' => enc.push('+'),
            _ => enc.push_str(&format!("%{:02X}", b)),
        }
    }
    format!("https://www.google.com/search?q={enc}")
}

pub fn open_google_search(query: &str) {
    if query.trim().is_empty() {
        return;
    }
    let url = HSTRING::from(build_google_search_url(query));
    unsafe {
        let _ = ShellExecuteW(
            Some(HWND::default()),
            PCWSTR::null(),
            &url,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}
