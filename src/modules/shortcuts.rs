use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::config::app_data_dir;
use crate::json_mini::{self, JsonVal};

#[derive(Debug, Clone)]
pub struct AppShortcut {
    pub id: u64,
    pub name: String,
    pub target: String,
}

pub struct ShortcutManager {
    pub items: Vec<AppShortcut>,
    next_id: u64,
}

impl ShortcutManager {
    pub fn load() -> Self {
        let path = app_data_dir().join("shortcuts.json");
        let Ok(raw) = fs::read_to_string(path) else {
            let mut mgr = Self {
                items: Vec::new(),
                next_id: 1,
            };
            mgr.pin("https://github.com", Some("GitHub"));
            mgr.pin("notepad.exe", Some("Notepad"));
            return mgr;
        };
        let Ok(JsonVal::Arr(arr)) = json_mini::parse(&raw) else {
            return Self {
                items: Vec::new(),
                next_id: 1,
            };
        };
        let mut items = Vec::new();
        let mut max_id = 0u64;
        for entry in arr {
            if let Some(map) = entry.as_obj() {
                let id = map.get("id").and_then(JsonVal::as_f64).unwrap_or(0.0) as u64;
                let name = map
                    .get("name")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string();
                let target = map
                    .get("target")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string();
                if id > 0 && !target.is_empty() {
                    max_id = max_id.max(id);
                    items.push(AppShortcut { id, name, target });
                }
            }
        }
        Self {
            items,
            next_id: max_id + 1,
        }
    }

    pub fn save(&self) {
        let arr: Vec<JsonVal> = self
            .items
            .iter()
            .map(|s| {
                let mut m = BTreeMap::new();
                m.insert("id".into(), JsonVal::Num(s.id as f64));
                m.insert("name".into(), JsonVal::Str(s.name.clone()));
                m.insert("target".into(), JsonVal::Str(s.target.clone()));
                JsonVal::Obj(m)
            })
            .collect();
        let path = app_data_dir().join("shortcuts.json");
        let _ = fs::write(path, json_mini::to_string_pretty(&JsonVal::Arr(arr)));
    }

    pub fn pin(&mut self, target: &str, name: Option<&str>) {
        let t = target.trim();
        if t.is_empty() {
            return;
        }
        let label = name
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                Path::new(t)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(t)
                    .to_string()
            });
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(AppShortcut {
            id,
            name: label,
            target: t.to_string(),
        });
        self.save();
    }

    pub fn remove(&mut self, id: u64) {
        self.items.retain(|s| s.id != id);
        self.save();
    }

    pub fn launch(&self, target: &str) {
        let t = HSTRING::from(target);
        unsafe {
            let _ = ShellExecuteW(
                Some(HWND::default()),
                PCWSTR::null(),
                &t,
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}
