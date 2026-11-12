use std::fs;
use std::path::Path;

use crate::modules::tasks::TaskManager;

#[derive(Debug, Clone)]
pub struct AgendaItem {
    pub title: String,
    pub time_label: String,
    pub source: &'static str,
}

#[derive(Default)]
pub struct CalendarManager {
    pub ics_events: Vec<AgendaItem>,
}

impl CalendarManager {
    pub fn import_ics_file(&mut self, path: &Path) -> usize {
        let Ok(raw) = fs::read_to_string(path) else {
            return 0;
        };
        let mut count = 0;
        let mut in_event = false;
        let mut summary = String::new();
        let mut dtstart = String::new();

        for line in raw.lines().map(str::trim) {
            if line == "BEGIN:VEVENT" {
                in_event = true;
                summary.clear();
                dtstart.clear();
            } else if line == "END:VEVENT" && in_event {
                in_event = false;
                if !summary.is_empty() {
                    let time_label = if dtstart.len() >= 13 {
                        format!("{}:{}", &dtstart[9..11], &dtstart[11..13])
                    } else {
                        "All day".into()
                    };
                    self.ics_events.push(AgendaItem {
                        title: summary.clone(),
                        time_label,
                        source: "ICS",
                    });
                    count += 1;
                }
            } else if in_event {
                if let Some(v) = line.strip_prefix("SUMMARY:") {
                    summary = v.trim().to_string();
                } else if let Some((_, v)) = line.split_once(':') {
                    if line.starts_with("DTSTART") {
                        dtstart = v.trim().to_string();
                    }
                }
            }
        }
        count
    }

    pub fn agenda(&self, tasks: &TaskManager) -> Vec<AgendaItem> {
        let mut out = self.ics_events.clone();
        for t in &tasks.items {
            if !t.done {
                out.push(AgendaItem {
                    title: t.title.clone(),
                    time_label: if t.due_label.is_empty() {
                        "Today".into()
                    } else {
                        t.due_label.clone()
                    },
                    source: "Task",
                });
            }
        }
        out
    }
}
