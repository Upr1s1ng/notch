use std::fs;
use std::path::Path;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

#[derive(Debug, Clone)]
pub struct ShelfItem {
    pub path: String,
    pub name: String,
    pub size_label: String,
}

#[derive(Default)]
pub struct FileShelf {
    pub items: Vec<ShelfItem>,
}

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", (bytes as f64) / 1024.0)
    } else {
        format!("{:.1} MB", (bytes as f64) / (1024.0 * 1024.0))
    }
}

impl FileShelf {
    /// Stores file path references only (never copies files into RAM or temp folders).
    pub fn add_paths(&mut self, paths: &[String]) -> usize {
        let mut added = 0;
        for raw in paths {
            let p = Path::new(raw);
            if !p.exists() {
                continue;
            }
            if self
                .items
                .iter()
                .any(|it| it.path.eq_ignore_ascii_case(raw))
            {
                continue;
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(raw)
                .to_string();
            let size_label = if p.is_dir() {
                "Folder".into()
            } else {
                fs::metadata(p)
                    .map(|m| fmt_size(m.len()))
                    .unwrap_or_else(|_| "File".into())
            };
            self.items.push(ShelfItem {
                path: raw.clone(),
                name,
                size_label,
            });
            added += 1;
        }
        added
    }

    pub fn remove_path(&mut self, path: &str) {
        self.items.retain(|it| !it.path.eq_ignore_ascii_case(path));
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn open_item(&self, path: &str) {
        let target = HSTRING::from(path);
        unsafe {
            let _ = ShellExecuteW(
                Some(HWND::default()),
                PCWSTR::null(),
                &target,
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}
