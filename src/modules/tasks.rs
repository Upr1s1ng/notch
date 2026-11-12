use std::collections::BTreeMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::app_data_dir;
use crate::json_mini::{self, JsonVal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    Today,
    Upcoming,
    Completed,
}

#[derive(Debug, Clone)]
pub struct TaskItem {
    pub id: u64,
    pub title: String,
    pub done: bool,
    pub due_label: String,
    pub due_ts: Option<u64>,
    pub reminder_fired: bool,
}

pub struct TaskManager {
    pub items: Vec<TaskItem>,
    next_id: u64,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TaskManager {
    pub fn load() -> Self {
        let path = app_data_dir().join("tasks.json");
        let Ok(raw) = fs::read_to_string(path) else {
            return Self {
                items: Vec::new(),
                next_id: 1,
            };
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
                let title = map
                    .get("title")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string();
                let done = map.get("done").and_then(JsonVal::as_bool).unwrap_or(false);
                let due_label = map
                    .get("due_label")
                    .and_then(JsonVal::as_str)
                    .unwrap_or("")
                    .to_string();
                let due_ts = map
                    .get("due_ts")
                    .and_then(JsonVal::as_f64)
                    .map(|n| n as u64);
                let reminder_fired = map
                    .get("reminder_fired")
                    .and_then(JsonVal::as_bool)
                    .unwrap_or(false);
                if id > 0 && !title.is_empty() {
                    max_id = max_id.max(id);
                    items.push(TaskItem {
                        id,
                        title,
                        done,
                        due_label,
                        due_ts,
                        reminder_fired,
                    });
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
            .map(|t| {
                let mut m = BTreeMap::new();
                m.insert("id".into(), JsonVal::Num(t.id as f64));
                m.insert("title".into(), JsonVal::Str(t.title.clone()));
                m.insert("done".into(), JsonVal::Bool(t.done));
                m.insert("due_label".into(), JsonVal::Str(t.due_label.clone()));
                if let Some(ts) = t.due_ts {
                    m.insert("due_ts".into(), JsonVal::Num(ts as f64));
                }
                m.insert("reminder_fired".into(), JsonVal::Bool(t.reminder_fired));
                JsonVal::Obj(m)
            })
            .collect();
        let path = app_data_dir().join("tasks.json");
        let _ = fs::write(path, json_mini::to_string_pretty(&JsonVal::Arr(arr)));
    }

    pub fn add_quick(&mut self, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut title = trimmed.to_string();
        let mut due_label = String::new();
        let mut due_ts = None;
        if let Some((left, right)) = trimmed.rsplit_once(' ') {
            if let Some((hh, mm)) = right.split_once(':') {
                if let (Ok(h), Ok(m)) = (hh.parse::<u32>(), mm.parse::<u32>()) {
                    if h < 24 && m < 60 && !left.trim().is_empty() {
                        title = left.trim().to_string();
                        due_label = format!("{h:02}:{m:02}");
                        due_ts = Some(now_unix() + 60);
                    }
                }
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(TaskItem {
            id,
            title,
            done: false,
            due_label,
            due_ts,
            reminder_fired: false,
        });
        self.save();
    }

    pub fn toggle_done(&mut self, id: u64) {
        if let Some(t) = self.items.iter_mut().find(|t| t.id == id) {
            t.done = !t.done;
            self.save();
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
        self.save();
    }

    pub fn filtered(&self, filter: TaskFilter) -> Vec<&TaskItem> {
        self.items
            .iter()
            .filter(|t| match filter {
                TaskFilter::Today => !t.done,
                TaskFilter::Upcoming => !t.done && !t.due_label.is_empty(),
                TaskFilter::Completed => t.done,
            })
            .collect()
    }

    pub fn pop_due_reminder(&mut self) -> Option<String> {
        let now = now_unix();
        for t in &mut self.items {
            if !t.done && !t.reminder_fired {
                if let Some(ts) = t.due_ts {
                    if ts <= now {
                        t.reminder_fired = true;
                        let title = t.title.clone();
                        self.save();
                        return Some(title);
                    }
                }
            }
        }
        None
    }
}
