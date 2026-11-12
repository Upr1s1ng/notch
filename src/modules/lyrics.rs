use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
    WINHTTP_FLAG_SECURE,
};

use crate::config::app_data_dir;
use crate::json_mini::{self, JsonVal};

#[derive(Debug, Clone)]
pub struct LrcLine {
    pub time_secs: f64,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct LyricsResult {
    pub track_key: String,
    pub synced: Vec<LrcLine>,
    pub plain: Vec<String>,
    pub loading: bool,
    pub error: String,
}

pub fn parse_lrc(raw: &str) -> Vec<LrcLine> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some((ts, txt)) = rest.split_once(']') {
                if let Some((mm_s, ss_s)) = ts.split_once(':') {
                    if let (Ok(mm), Ok(ss)) = (mm_s.parse::<f64>(), ss_s.parse::<f64>()) {
                        let text = txt.trim().to_string();
                        if !text.is_empty() {
                            out.push(LrcLine {
                                time_secs: mm * 60.0 + ss,
                                text,
                            });
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.time_secs
            .partial_cmp(&b.time_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub fn active_line_index(lines: &[LrcLine], pos_secs: f64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let mut idx = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.time_secs <= pos_secs + 0.15 {
            idx = i;
        } else {
            break;
        }
    }
    Some(idx)
}

fn cache_path(track_key: &str) -> PathBuf {
    let dir = app_data_dir().join("lyrics_cache");
    let _ = fs::create_dir_all(&dir);
    let mut hash: u64 = 14695981039346656037;
    for b in track_key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    dir.join(format!("{hash:016x}.json"))
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn winhttp_get(host: &str, path_and_query: &str) -> Result<String, String> {
    unsafe {
        let agent = HSTRING::from("NotchWindows/0.1");
        let h_session = WinHttpOpen(
            &agent,
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if h_session.is_null() {
            return Err("WinHttpOpen failed".into());
        }
        let h_host = HSTRING::from(host);
        let h_conn = WinHttpConnect(h_session, &h_host, 443, 0);
        if h_conn.is_null() {
            let _ = WinHttpCloseHandle(h_session);
            return Err("WinHttpConnect failed".into());
        }
        let h_verb = HSTRING::from("GET");
        let h_path = HSTRING::from(path_and_query);
        let h_req = WinHttpOpenRequest(
            h_conn,
            &h_verb,
            &h_path,
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if h_req.is_null() {
            let _ = WinHttpCloseHandle(h_conn);
            let _ = WinHttpCloseHandle(h_session);
            return Err("WinHttpOpenRequest failed".into());
        }
        let ok = WinHttpSendRequest(h_req, None, None, 0, 0, 0).is_ok()
            && WinHttpReceiveResponse(h_req, std::ptr::null_mut()).is_ok();
        let mut body = Vec::new();
        if ok {
            loop {
                let mut avail: u32 = 0;
                if WinHttpQueryDataAvailable(h_req, &mut avail).is_err() || avail == 0 {
                    break;
                }
                let mut buf = vec![0u8; avail as usize];
                let mut read: u32 = 0;
                if WinHttpReadData(h_req, buf.as_mut_ptr() as *mut _, avail, &mut read).is_err()
                    || read == 0
                {
                    break;
                }
                body.extend_from_slice(&buf[..read as usize]);
            }
        }
        let _ = WinHttpCloseHandle(h_req);
        let _ = WinHttpCloseHandle(h_conn);
        let _ = WinHttpCloseHandle(h_session);
        if body.is_empty() {
            return Err("empty response".into());
        }
        String::from_utf8(body).map_err(|_| "invalid utf8".into())
    }
}

pub fn fetch_lyrics_async(
    title: String,
    artist: String,
    album: String,
    duration_secs: f64,
    tx: Sender<LyricsResult>,
) {
    let key = format!("{title}|{artist}|{album}");
    if title.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let cpath = cache_path(&key);
        if let Ok(cached) = fs::read_to_string(&cpath) {
            if let Some(res) = decode_lrclib_json(&key, &cached) {
                let _ = tx.send(res);
                return;
            }
        }
        let query = format!(
            "/api/get?track_name={}&artist_name={}&album_name={}&duration={}",
            url_encode(&title),
            url_encode(&artist),
            url_encode(&album),
            duration_secs.round() as i64
        );
        match winhttp_get("lrclib.net", &query) {
            Ok(body) => {
                let _ = fs::write(&cpath, &body);
                if let Some(res) = decode_lrclib_json(&key, &body) {
                    let _ = tx.send(res);
                } else {
                    let _ = tx.send(LyricsResult {
                        track_key: key,
                        error: "No lyrics found".into(),
                        ..Default::default()
                    });
                }
            }
            Err(e) => {
                let _ = tx.send(LyricsResult {
                    track_key: key,
                    error: e,
                    ..Default::default()
                });
            }
        }
    });
}

fn decode_lrclib_json(track_key: &str, raw: &str) -> Option<LyricsResult> {
    let JsonVal::Obj(map) = json_mini::parse(raw).ok()? else {
        return None;
    };
    let synced_raw = map
        .get("syncedLyrics")
        .and_then(JsonVal::as_str)
        .unwrap_or("");
    let plain_raw = map
        .get("plainLyrics")
        .and_then(JsonVal::as_str)
        .unwrap_or("");
    let synced = parse_lrc(synced_raw);
    let plain: Vec<String> = plain_raw
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if synced.is_empty() && plain.is_empty() {
        return None;
    }
    let _ = BTreeMap::<String, JsonVal>::new();
    Some(LyricsResult {
        track_key: track_key.to_string(),
        synced,
        plain,
        loading: false,
        error: String::new(),
    })
}
