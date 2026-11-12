use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmEnableBlurBehindWindow, DwmExtendFrameIntoClientArea, DWM_BB_ENABLE, DWM_BLURBEHIND,
};
use windows::Win32::Graphics::Gdi::{
    CreateRoundRectRgn, GetMonitorInfoW, MonitorFromPoint, SetWindowRgn, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, TrackMouseEvent, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
    TME_LEAVE, TRACKMOUSEEVENT, VK_BACK, VK_ESCAPE, VK_RETURN, VK_SPACE,
};
use windows::Win32::UI::Shell::{
    DragAcceptFiles, DragFinish, DragQueryFileW, SHQueryUserNotificationState, Shell_NotifyIconW,
    HDROP, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, QUNS_BUSY,
    QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW, GetWindowLongPtrW,
    GetWindowRect, KillTimer, LoadCursorW, LoadIconW, MessageBoxW, PostMessageW, PostQuitMessage,
    RegisterClassExW, SetForegroundWindow, SetTimer, SetWindowDisplayAffinity, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TrackPopupMenu, TranslateMessage, EVENT_SYSTEM_FOREGROUND,
    GWL_EXSTYLE, HMENU, HWND_TOPMOST, IDC_ARROW, IDI_APPLICATION, MA_ACTIVATE, MA_NOACTIVATE,
    MB_ICONINFORMATION, MB_OK, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNOACTIVATE, TPM_BOTTOMALIGN, TPM_LEFTALIGN, WA_INACTIVE, WDA_EXCLUDEFROMCAPTURE,
    WDA_NONE, WINEVENT_OUTOFCONTEXT, WM_ACTIVATE, WM_APP, WM_CHAR, WM_CLIPBOARDUPDATE, WM_CLOSE,
    WM_COMMAND, WM_DESTROY, WM_DPICHANGED, WM_DROPFILES, WM_HOTKEY, WM_KEYDOWN, WM_KILLFOCUS,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSELEAVE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT,
    WM_RBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::config::{AppConfig, NotchStyle, VisibilityMode};
use crate::modules::audio::{AppAudioSession, AudioController};
use crate::modules::brightness::BrightnessController;
use crate::modules::calendar::CalendarManager;
use crate::modules::clipboard::ClipboardHistory;
use crate::modules::discord::DiscordController;
use crate::modules::lyrics::{fetch_lyrics_async, LyricsResult};
use crate::modules::media::{MediaSnapshot, SmtcController};
use crate::modules::search::open_google_search;
use crate::modules::shelf::FileShelf;
use crate::modules::shortcuts::ShortcutManager;
use crate::modules::system_events::SystemEventMonitor;
use crate::modules::tasks::{TaskFilter, TaskManager};
use crate::modules::timer::TimerState;
use crate::os::d2d::D2dRenderer;
use crate::state::{ExpandedTab, NotchState, NotchStateMachine};
use crate::ui::shell::{draw_shell, ShellData, UiAction, UiHitMap};

const TIMER_ANIM_ID: usize = 1;
const TIMER_SLOW_ID: usize = 2;
const HOTKEY_TOGGLE_HIDE: i32 = 1;
const HOTKEY_SEARCH: i32 = 2;
const WM_TRAY: u32 = WM_APP + 1;

const IDM_TOGGLE: usize = 1001;
const IDM_AUTOSTART: usize = 1002;
const IDM_MASCOT: usize = 1003;
const IDM_MINIMAL: usize = 1004;
const IDM_STYLE_TOGGLE: usize = 1005;
const IDM_PRIVACY: usize = 1006;
const IDM_ABOUT: usize = 1007;
const IDM_EXIT: usize = 1008;

thread_local! {
    static ACTIVE_WINDOW: RefCell<Option<Rc<RefCell<WindowInner>>>> = const { RefCell::new(None) };
}

struct WindowInner {
    hwnd: HWND,
    backdrop_hwnd: HWND,
    cfg: AppConfig,
    sm: NotchStateMachine,
    d2d: D2dRenderer,
    hits: UiHitMap,
    dpi_scale: f32,
    mon_rect: RECT,
    anim_active: bool,
    focused_input: Option<u8>,
    tracking_mouse: bool,
    tray_added: bool,
    win_event_hook: HWINEVENTHOOK,

    smtc: SmtcController,
    audio: AudioController,
    brightness_ctrl: BrightnessController,
    discord: DiscordController,
    sys_events: SystemEventMonitor,
    tasks: TaskManager,
    calendar: CalendarManager,
    timer: TimerState,
    shortcuts: ShortcutManager,
    shelf: FileShelf,
    clipboard: ClipboardHistory,

    media_snap: MediaSnapshot,
    lyrics_res: LyricsResult,
    lyrics_tx: Sender<LyricsResult>,
    lyrics_rx: Receiver<LyricsResult>,
    last_track_key: String,
    show_lyrics: bool,
    master_vol: f32,
    master_muted: bool,
    brightness_val: f32,
    app_sessions: Vec<AppAudioSession>,
    search_text: String,
    task_input: String,
    app_input: String,
    task_filter: TaskFilter,
}

pub struct NotchWindow;

impl NotchWindow {
    pub fn run() -> windows::core::Result<()> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let cfg = AppConfig::load();
        let sm = NotchStateMachine::new(&cfg);
        let hinst = unsafe { GetModuleHandleW(None)? };

        let class_name = w!("NotchD2DWindowClass");
        let backdrop_class = w!("NotchGlassBackdropClass");

        unsafe {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                hIcon: LoadIconW(None, IDI_APPLICATION)?,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassExW(&wc);

            let bwc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: hinst.into(),
                lpszClassName: backdrop_class,
                ..Default::default()
            };
            RegisterClassExW(&bwc);
        }

        let mon_rect = primary_monitor_rect();
        let canvas_w = (cfg.expanded_width + 48.0).round() as i32;
        let canvas_h = (cfg.expanded_height + 44.0).round() as i32;
        let win_x = mon_rect.left + ((mon_rect.right - mon_rect.left) - canvas_w) / 2;
        let win_y = mon_rect.top;

        let backdrop_hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE
                    | WS_EX_LAYERED,
                backdrop_class,
                w!("NotchBackdrop"),
                WS_POPUP,
                win_x,
                win_y,
                1,
                1,
                None,
                None,
                Some(hinst.into()),
                None,
            )?
        };

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                class_name,
                w!("Notch"),
                WS_POPUP,
                win_x,
                win_y,
                canvas_w,
                canvas_h,
                None,
                None,
                Some(hinst.into()),
                None,
            )?
        };

        unsafe {
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
            let bb = DWM_BLURBEHIND {
                dwFlags: DWM_BB_ENABLE,
                fEnable: true.into(),
                ..Default::default()
            };
            let _ = DwmEnableBlurBehindWindow(backdrop_hwnd, &bb);
            if cfg.exclude_from_capture {
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
            }
            DragAcceptFiles(hwnd, true);
            let _ = AddClipboardFormatListener(hwnd);
        }

        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96) as f32;
        let dpi_scale = dpi / 96.0;
        let d2d = D2dRenderer::new(hwnd, canvas_w as u32, canvas_h as u32)?;
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };

        let audio = AudioController::new();
        let (master_vol, master_muted) = audio.get_master_volume();
        let brightness_ctrl = BrightnessController::new();
        let brightness_val = brightness_ctrl.get_brightness(Some(hwnd)).level;
        let discord = DiscordController::new(cfg.clone());
        discord.start();
        let (lyrics_tx, lyrics_rx) = mpsc::channel();

        let inner = Rc::new(RefCell::new(WindowInner {
            hwnd,
            backdrop_hwnd,
            cfg,
            sm,
            d2d,
            hits: UiHitMap::default(),
            dpi_scale,
            mon_rect,
            anim_active: false,
            focused_input: None,
            tracking_mouse: false,
            tray_added: false,
            win_event_hook: hook,
            smtc: SmtcController::new(),
            audio,
            brightness_ctrl,
            discord,
            sys_events: SystemEventMonitor::new(master_vol),
            tasks: TaskManager::load(),
            calendar: CalendarManager::default(),
            timer: TimerState::default(),
            shortcuts: ShortcutManager::load(),
            shelf: FileShelf::default(),
            clipboard: ClipboardHistory::default(),
            media_snap: MediaSnapshot::default(),
            lyrics_res: LyricsResult::default(),
            lyrics_tx,
            lyrics_rx,
            last_track_key: String::new(),
            show_lyrics: true,
            master_vol,
            master_muted,
            brightness_val,
            app_sessions: Vec::new(),
            search_text: String::new(),
            task_input: String::new(),
            app_input: String::new(),
            task_filter: TaskFilter::Today,
        }));

        ACTIVE_WINDOW.with(|slot| *slot.borrow_mut() = Some(inner.clone()));

        unsafe {
            let _ = RegisterHotKey(
                Some(hwnd),
                HOTKEY_TOGGLE_HIDE,
                HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0),
                b'D' as u32,
            );
            let _ = RegisterHotKey(
                Some(hwnd),
                HOTKEY_SEARCH,
                HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0),
                VK_SPACE.0 as u32,
            );
            SetTimer(Some(hwnd), TIMER_SLOW_ID, 1000, None);
        }

        inner.borrow_mut().add_tray_icon();
        inner.borrow_mut().refresh_heavy();
        inner.borrow_mut().sync_geometry_and_paint();
        unsafe {
            let _ = ShowWindow(backdrop_hwnd, SW_SHOWNOACTIVATE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            CoUninitialize();
        }
        Ok(())
    }
}

impl WindowInner {
    fn canvas_dip(&self) -> (f32, f32) {
        (
            self.cfg.expanded_width + 48.0,
            self.cfg.expanded_height + 44.0,
        )
    }

    fn set_anim_timer(&mut self, active: bool) {
        if active == self.anim_active {
            return;
        }
        self.anim_active = active;
        unsafe {
            if active {
                timeBeginPeriod(1);
                SetTimer(Some(self.hwnd), TIMER_ANIM_ID, 8, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), TIMER_ANIM_ID);
                timeEndPeriod(1);
            }
        }
    }

    fn refresh_heavy(&mut self) {
        let (v, m) = self.audio.get_master_volume();
        self.master_vol = v;
        self.master_muted = m;
        self.app_sessions = self.audio.list_app_sessions();
        self.brightness_val = self.brightness_ctrl.get_brightness(Some(self.hwnd)).level;
        let snap = self.smtc.read_snapshot();
        let tk = snap.track_key();
        if snap.has_session && tk != self.last_track_key {
            self.last_track_key = tk;
            fetch_lyrics_async(
                snap.title.clone(),
                snap.artist.clone(),
                snap.album.clone(),
                snap.duration_secs,
                self.lyrics_tx.clone(),
            );
            if self.sm.state == NotchState::Collapsed && !snap.title.is_empty() {
                self.sm.trigger_peek(&snap.title, &snap.artist, &self.cfg);
            }
        }
        self.media_snap = snap;
        while let Ok(res) = self.lyrics_rx.try_recv() {
            if res.track_key == self.last_track_key {
                self.lyrics_res = res;
            }
        }
    }

    fn sync_backdrop(&self) {
        if !self.cfg.dual_hwnd_glass || self.sm.state == NotchState::Hidden {
            unsafe {
                let _ = ShowWindow(self.backdrop_hwnd, SW_HIDE);
            }
            return;
        }
        let w_px = (self.sm.width.value * self.dpi_scale).round() as i32;
        let h_px = (self.sm.height.value * self.dpi_scale).round() as i32;
        let r_px = (self.sm.corner_radius.value * 2.0 * self.dpi_scale).round() as i32;
        let x_px = self.mon_rect.left + (((self.mon_rect.right - self.mon_rect.left) - w_px) / 2);
        let y_px = self.mon_rect.top + (self.sm.top_offset.value * self.dpi_scale).round() as i32;
        unsafe {
            let _ = ShowWindow(self.backdrop_hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                self.backdrop_hwnd,
                Some(self.hwnd),
                x_px,
                y_px,
                w_px.max(1),
                h_px.max(1),
                SWP_NOACTIVATE,
            );
            let rgn = CreateRoundRectRgn(0, 0, w_px + 1, h_px + 1, r_px, r_px);
            let _ = SetWindowRgn(self.backdrop_hwnd, Some(rgn), true);
        }
    }

    fn sync_geometry_and_paint(&mut self) {
        if self.sm.state == NotchState::Hidden {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                let _ = ShowWindow(self.backdrop_hwnd, SW_HIDE);
            }
            return;
        }
        self.sync_backdrop();
        let (cw, _) = self.canvas_dip();
        let dc_state = self.discord.snapshot();
        let filtered_tasks = self.tasks.filtered(self.task_filter);
        let agenda = self.calendar.agenda(&self.tasks);
        let data = ShellData {
            media: &self.media_snap,
            lyrics: &self.lyrics_res,
            show_lyrics: self.show_lyrics,
            master_vol: self.master_vol,
            master_muted: self.master_muted,
            brightness: self.brightness_val,
            app_sessions: &self.app_sessions,
            discord: &dc_state,
            search_text: &self.search_text,
            task_input: &self.task_input,
            app_input: &self.app_input,
            focused_input: self.focused_input,
            task_filter: self.task_filter,
            tasks: &filtered_tasks,
            agenda: &agenda,
            timer: &self.timer,
            shortcuts: &self.shortcuts.items,
            shelf: &self.shelf.items,
            clips: &self.clipboard.entries,
        };
        self.d2d.begin_draw();
        draw_shell(&self.d2d, &self.sm, &self.cfg, &data, cw, &mut self.hits);
        let _ = self.d2d.end_draw();
    }

    fn tick(&mut self) {
        let out = self.sm.tick(&self.cfg);
        self.set_anim_timer(out.needs_timer);
        if out.needs_repaint {
            self.sync_geometry_and_paint();
        }
    }

    fn set_smart_focus(&mut self, input_id: Option<u8>) {
        self.focused_input = input_id;
        let enabled = input_id.is_some();
        self.sm.input_focused = enabled;
        unsafe {
            let ex = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            if enabled {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex & !(WS_EX_NOACTIVATE.0 as isize));
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
                let _ = SetForegroundWindow(self.hwnd);
            } else {
                SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 as isize));
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
        self.sync_geometry_and_paint();
    }

    fn active_input_mut(&mut self) -> Option<&mut String> {
        match self.focused_input {
            Some(0) => Some(&mut self.search_text),
            Some(1) => Some(&mut self.task_input),
            Some(2) => Some(&mut self.app_input),
            _ => None,
        }
    }

    fn check_fullscreen(&mut self) {
        if !self.cfg.hide_in_fullscreen {
            return;
        }
        let mut is_fs = false;
        unsafe {
            if let Ok(state) = SHQueryUserNotificationState() {
                if state == QUNS_BUSY || state == QUNS_RUNNING_D3D_FULL_SCREEN {
                    is_fs = true;
                }
            }
            if !is_fs {
                let fg = GetForegroundWindow();
                if !fg.is_invalid() && fg != self.hwnd && fg != self.backdrop_hwnd {
                    let mut rc = RECT::default();
                    if GetWindowRect(fg, &mut rc).is_ok()
                        && rc.left <= self.mon_rect.left
                        && rc.top <= self.mon_rect.top
                        && rc.right >= self.mon_rect.right
                        && rc.bottom >= self.mon_rect.bottom
                    {
                        is_fs = true;
                    }
                }
            }
        }
        let was_hidden = self.sm.state == NotchState::Hidden;
        self.sm.set_fullscreen_hidden(is_fs, &self.cfg);
        if self.sm.state == NotchState::Hidden {
            self.set_anim_timer(false);
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                let _ = ShowWindow(self.backdrop_hwnd, SW_HIDE);
            }
        } else if was_hidden {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            self.sync_geometry_and_paint();
        }
    }

    fn toggle_privacy(&mut self) {
        self.cfg.exclude_from_capture = !self.cfg.exclude_from_capture;
        unsafe {
            let _ = SetWindowDisplayAffinity(
                self.hwnd,
                if self.cfg.exclude_from_capture {
                    WDA_EXCLUDEFROMCAPTURE
                } else {
                    WDA_NONE
                },
            );
        }
        self.cfg.save();
        self.sync_geometry_and_paint();
    }

    fn handle_action(&mut self, act: UiAction) {
        match act {
            UiAction::ToggleExpand => {
                self.sm.toggle_expand(&self.cfg);
                if self.sm.state == NotchState::Expanded {
                    self.refresh_heavy();
                }
            }
            UiAction::Collapse => {
                self.set_smart_focus(None);
                self.sm.pinned_open = false;
                self.sm.collapse(&self.cfg);
            }
            UiAction::TogglePin => self.sm.pinned_open = !self.sm.pinned_open,
            UiAction::SelectTab(tab) => {
                self.set_smart_focus(None);
                self.sm.expand(tab, &self.cfg);
                if matches!(
                    tab,
                    ExpandedTab::Media | ExpandedTab::Audio | ExpandedTab::Home
                ) {
                    self.refresh_heavy();
                }
            }
            UiAction::CycleStyle => {
                self.cfg.style = match self.cfg.style {
                    NotchStyle::TopEdge => NotchStyle::FloatingPill,
                    NotchStyle::FloatingPill => NotchStyle::Minimal,
                    NotchStyle::Minimal => NotchStyle::TopEdge,
                };
                self.cfg.save();
                self.sm.apply_targets(&self.cfg);
            }
            UiAction::ToggleMascot => {
                self.cfg.mascot_enabled = !self.cfg.mascot_enabled;
                self.cfg.save();
            }
            UiAction::TogglePrivacy => self.toggle_privacy(),
            UiAction::TriggerPeekDemo => {
                self.sm
                    .trigger_peek("Notch Peek", "2.5s auto-collapse banner", &self.cfg)
            }
            UiAction::FocusInput(id) => self.set_smart_focus(Some(id)),
            UiAction::SubmitSearch => {
                let q = self.search_text.trim().to_string();
                if !q.is_empty() {
                    open_google_search(&q);
                    self.search_text.clear();
                    self.set_smart_focus(None);
                    self.sm.collapse(&self.cfg);
                }
            }
            UiAction::DiscordMute => self.discord.toggle_mute(&self.audio),
            UiAction::DiscordDeafen => self.discord.toggle_deafen(&self.audio),
            UiAction::DiscordDisconnect => self.discord.disconnect_call(),
            UiAction::MediaTogglePlay => {
                self.smtc.toggle_play_pause();
                self.refresh_heavy();
            }
            UiAction::MediaNext => {
                self.smtc.next_track();
                self.refresh_heavy();
            }
            UiAction::MediaPrev => {
                self.smtc.prev_track();
                self.refresh_heavy();
            }
            UiAction::MediaCycleSource => {
                self.smtc.cycle_source();
                self.refresh_heavy();
            }
            UiAction::MediaToggleLyrics => self.show_lyrics = !self.show_lyrics,
            UiAction::SeekLyric(ms) => self.smtc.seek_to_seconds((ms as f64) / 1000.0),
            UiAction::SeekRatio(r) => {
                if self.media_snap.duration_secs > 0.0 {
                    self.smtc
                        .seek_to_seconds(self.media_snap.duration_secs * (r as f64));
                }
            }
            UiAction::SetMasterVol(r) => {
                self.audio.set_master_volume(r);
                self.master_vol = r;
            }
            UiAction::ToggleMasterMute => {
                self.master_muted = self.audio.toggle_master_mute();
            }
            UiAction::SetBrightness(r) => {
                self.brightness_ctrl.set_brightness(r, Some(self.hwnd));
                self.brightness_val = r;
            }
            UiAction::SetAppVol(pid, r) => {
                self.audio.set_app_volume(pid, r);
                if let Some(s) = self.app_sessions.iter_mut().find(|s| s.pid == pid) {
                    s.volume = r;
                }
            }
            UiAction::SetTaskFilter(f) => self.task_filter = f,
            UiAction::AddTask => {
                let raw = self.task_input.clone();
                self.tasks.add_quick(&raw);
                self.task_input.clear();
                self.set_smart_focus(None);
            }
            UiAction::ToggleTask(id) => self.tasks.toggle_done(id),
            UiAction::DeleteTask(id) => self.tasks.delete(id),
            UiAction::TimerToggle => self.timer.toggle(),
            UiAction::TimerReset => self.timer.reset(),
            UiAction::TimerPreset(s) => self.timer.set_preset(
                s,
                if s == 300 {
                    "Short Break"
                } else {
                    "Pomodoro Focus"
                },
            ),
            UiAction::LaunchShortcut(idx) => {
                if let Some(sc) = self.shortcuts.items.get(idx) {
                    self.shortcuts.launch(&sc.target);
                }
            }
            UiAction::RemoveShortcut(id) => self.shortcuts.remove(id),
            UiAction::AddShortcut => {
                let raw = self.app_input.clone();
                self.shortcuts.pin(&raw, None);
                self.app_input.clear();
                self.set_smart_focus(None);
            }
            UiAction::OpenShelfItem(idx) => {
                if let Some(it) = self.shelf.items.get(idx) {
                    self.shelf.open_item(&it.path);
                }
            }
            UiAction::RemoveShelfItem(idx) => {
                if let Some(it) = self.shelf.items.get(idx).cloned() {
                    self.shelf.remove_path(&it.path);
                }
            }
            UiAction::ClearShelf => self.shelf.clear(),
            UiAction::CopyClip(idx) => {
                if let Some(c) = self.clipboard.entries.get(idx) {
                    self.clipboard.copy_to_clipboard(self.hwnd, &c.text);
                }
            }
        }
        self.tick();
        self.sync_geometry_and_paint();
    }

    fn add_tray_icon(&mut self) {
        if self.tray_added {
            return;
        }
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
                ..Default::default()
            };
            let tip: Vec<u16> = "Notch — Dynamic Island".encode_utf16().collect();
            let len = tip.len().min(nid.szTip.len() - 1);
            nid.szTip[..len].copy_from_slice(&tip[..len]);
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
            self.tray_added = true;
        }
    }

    fn remove_tray_icon(&mut self) {
        if !self.tray_added {
            return;
        }
        unsafe {
            let nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            self.tray_added = false;
        }
    }

    fn show_tray_menu(&self) {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let _ = AppendMenuW(menu, MF_STRING, IDM_TOGGLE, w!("Expand / Collapse Notch"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if is_autostart_enabled() {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
                IDM_AUTOSTART,
                w!("Start with Windows"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if self.cfg.mascot_enabled {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
                IDM_MASCOT,
                w!("Nusa Mascot"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if self.cfg.style == NotchStyle::Minimal {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
                IDM_MINIMAL,
                w!("Minimal Notch"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                IDM_STYLE_TOGGLE,
                w!("Switch Top-Edge / Floating Pill"),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING
                    | if self.cfg.exclude_from_capture {
                        MF_CHECKED
                    } else {
                        MF_UNCHECKED
                    },
                IDM_PRIVACY,
                w!("Hide from Screen Capture"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, IDM_ABOUT, w!("About Notch"));
            let _ = AppendMenuW(menu, MF_STRING, IDM_EXIT, w!("Exit"));

            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                pt.x,
                pt.y,
                Some(0),
                self.hwnd,
                None,
            );
            let _ = DestroyMenu(menu);
        }
    }

    fn cleanup(&mut self) {
        self.set_anim_timer(false);
        self.discord.stop();
        self.remove_tray_icon();
        unsafe {
            let _ = RemoveClipboardFormatListener(self.hwnd);
            let _ = KillTimer(Some(self.hwnd), TIMER_SLOW_ID);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_TOGGLE_HIDE);
            let _ = UnregisterHotKey(Some(self.hwnd), HOTKEY_SEARCH);
            if !self.win_event_hook.is_invalid() {
                let _ = UnhookWinEvent(self.win_event_hook);
            }
            let _ = DestroyWindow(self.backdrop_hwnd);
        }
    }
}

fn primary_monitor_rect() -> RECT {
    unsafe {
        let hmon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut mi).as_bool() {
            return mi.rcMonitor;
        }
    }
    RECT {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    }
}

fn is_autostart_enabled() -> bool {
    unsafe {
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let ok = RegQueryValueExW(key, w!("Notch"), None, None, None, None).is_ok();
        let _ = RegCloseKey(key);
        ok
    }
}

fn toggle_autostart() {
    unsafe {
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            KEY_READ | KEY_WRITE,
            &mut key,
        )
        .is_err()
        {
            return;
        }
        if is_autostart_enabled() {
            let _ = RegDeleteValueW(key, w!("Notch"));
        } else if let Ok(exe) = std::env::current_exe() {
            let val = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            let _ = RegSetValueExW(key, w!("Notch"), Some(0), REG_SZ, Some(bytes));
        }
        let _ = RegCloseKey(key);
    }
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_obj: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    ACTIVE_WINDOW.with(|slot| {
        if let Some(rc) = slot.borrow().as_ref() {
            if let Ok(mut w) = rc.try_borrow_mut() {
                w.check_fullscreen();
            }
        }
    });
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let rc_opt = ACTIVE_WINDOW.with(|slot| slot.borrow().clone());
    let Some(rc) = rc_opt else {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    let mut w = match rc.try_borrow_mut() {
        Ok(guard) => guard,
        Err(_) => return DefWindowProcW(hwnd, msg, wparam, lparam),
    };

    match msg {
        WM_MOUSEACTIVATE => LRESULT(if w.focused_input.is_some() {
            MA_ACTIVATE as isize
        } else {
            MA_NOACTIVATE as isize
        }),
        WM_ACTIVATE
            if (wparam.0 & 0xFFFF) == (WA_INACTIVE as usize) && w.focused_input.is_some() =>
        {
            w.set_smart_focus(None);
            LRESULT(0)
        }
        WM_KILLFOCUS if w.focused_input.is_some() => {
            w.set_smart_focus(None);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !w.tracking_mouse {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                w.tracking_mouse = true;
            }
            let px = ((lparam.0 & 0xFFFF) as i16) as f32 / w.dpi_scale;
            let py = (((lparam.0 >> 16) & 0xFFFF) as i16) as f32 / w.dpi_scale;
            let cfg = w.cfg.clone();
            w.sm.on_hover_change(true, &cfg);
            let hovered = w.hits.update_hover(px, py);
            w.tick();
            if hovered {
                w.sync_geometry_and_paint();
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            w.tracking_mouse = false;
            w.hits.hovered_idx = None;
            let cfg = w.cfg.clone();
            w.sm.on_hover_change(false, &cfg);
            w.tick();
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let px = ((lparam.0 & 0xFFFF) as i16) as f32 / w.dpi_scale;
            let py = (((lparam.0 >> 16) & 0xFFFF) as i16) as f32 / w.dpi_scale;
            if let Some(act) = w.hits.hit_test(px, py) {
                w.handle_action(act);
            } else if w.focused_input.is_some() {
                w.set_smart_focus(None);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL if w.sm.state == NotchState::Expanded => {
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
            let cur = ExpandedTab::ALL
                .iter()
                .position(|t| *t == w.sm.active_tab)
                .unwrap_or(0);
            let next = if delta < 0 {
                (cur + 1) % ExpandedTab::ALL.len()
            } else {
                (cur + ExpandedTab::ALL.len() - 1) % ExpandedTab::ALL.len()
            };
            w.handle_action(UiAction::SelectTab(ExpandedTab::ALL[next]));
            LRESULT(0)
        }
        WM_KEYDOWN if w.focused_input.is_some() => {
            if wparam.0 == (VK_ESCAPE.0 as usize) {
                w.set_smart_focus(None);
            } else if wparam.0 == (VK_RETURN.0 as usize) {
                match w.focused_input {
                    Some(0) => w.handle_action(UiAction::SubmitSearch),
                    Some(1) => w.handle_action(UiAction::AddTask),
                    Some(2) => w.handle_action(UiAction::AddShortcut),
                    _ => w.set_smart_focus(None),
                }
            } else if wparam.0 == (VK_BACK.0 as usize) {
                if let Some(s) = w.active_input_mut() {
                    s.pop();
                }
                w.sync_geometry_and_paint();
            }
            LRESULT(0)
        }
        WM_CHAR if w.focused_input.is_some() => {
            if let Some(ch) = char::from_u32(wparam.0 as u32) {
                if !ch.is_control() {
                    if let Some(s) = w.active_input_mut() {
                        s.push(ch);
                    }
                    w.sync_geometry_and_paint();
                }
            }
            LRESULT(0)
        }
        WM_DROPFILES => {
            let hdrop = HDROP(wparam.0 as *mut _);
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            let mut paths = Vec::new();
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    DragQueryFileW(hdrop, i, Some(&mut buf));
                    paths.push(String::from_utf16_lossy(&buf[..len as usize]));
                }
            }
            DragFinish(hdrop);
            if w.shelf.add_paths(&paths) > 0 {
                let cfg = w.cfg.clone();
                w.sm.expand(ExpandedTab::Shelf, &cfg);
                w.tick();
                w.sync_geometry_and_paint();
            }
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            w.clipboard.capture_if_safe(hwnd);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_ANIM_ID {
                w.tick();
            } else if wparam.0 == TIMER_SLOW_ID {
                if let Some(due_title) = w.tasks.pop_due_reminder() {
                    let cfg = w.cfg.clone();
                    w.sm.trigger_peek("Task Reminder", due_title, &cfg);
                    w.tick();
                }
                if w.timer.tick() {
                    let cfg = w.cfg.clone();
                    w.sm.trigger_peek("Timer Done", w.timer.label.clone(), &cfg);
                    w.tick();
                }
                let (v, _) = w.audio.get_master_volume();
                if let Some((pt, ps)) = w.sys_events.poll(v) {
                    let cfg = w.cfg.clone();
                    w.sm.trigger_peek(pt, ps, &cfg);
                    w.tick();
                }
                if w.sm.state == NotchState::Expanded || w.timer.running || w.media_snap.is_playing
                {
                    w.refresh_heavy();
                    w.sync_geometry_and_paint();
                } else {
                    let prev_mood = w.sm.mascot_mood;
                    let cfg = w.cfg.clone();
                    let has_live = w.media_snap.is_playing || w.timer.running;
                    w.sm.update_mascot_idle(&cfg, has_live);
                    if w.sm.mascot_mood != prev_mood {
                        w.sync_geometry_and_paint();
                    }
                }
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            let cfg = w.cfg.clone();
            if wparam.0 == HOTKEY_TOGGLE_HIDE as usize {
                w.sm.toggle_manual_hide(&cfg);
                w.sync_geometry_and_paint();
            } else if wparam.0 == HOTKEY_SEARCH as usize {
                w.sm.manual_hidden = false;
                w.sm.expand(ExpandedTab::Home, &cfg);
                w.tick();
                w.set_smart_focus(Some(0));
            }
            LRESULT(0)
        }
        WM_TRAY if lparam.0 as u32 == WM_RBUTTONUP => {
            w.show_tray_menu();
            LRESULT(0)
        }
        WM_COMMAND => {
            match wparam.0 & 0xFFFF {
                IDM_TOGGLE => w.handle_action(UiAction::ToggleExpand),
                IDM_AUTOSTART => toggle_autostart(),
                IDM_MASCOT => w.handle_action(UiAction::ToggleMascot),
                IDM_MINIMAL => {
                    w.cfg.style = if w.cfg.style == NotchStyle::Minimal {
                        NotchStyle::TopEdge
                    } else {
                        NotchStyle::Minimal
                    };
                    w.cfg.save();
                    let cfg = w.cfg.clone();
                    w.sm.apply_targets(&cfg);
                    w.tick();
                }
                IDM_STYLE_TOGGLE => {
                    w.cfg.style = if w.cfg.style == NotchStyle::TopEdge {
                        NotchStyle::FloatingPill
                    } else {
                        NotchStyle::TopEdge
                    };
                    w.cfg.save();
                    let cfg = w.cfg.clone();
                    w.sm.apply_targets(&cfg);
                    w.tick();
                }
                IDM_PRIVACY => w.toggle_privacy(),
                IDM_ABOUT => {
                    let _ = MessageBoxW(
                        Some(hwnd),
                        w!("Notch v0.1.0\nPure Rust + Direct2D Dynamic Island for Windows 10/11."),
                        w!("About Notch"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                IDM_EXIT => {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF).max(96) as f32;
            w.dpi_scale = dpi / 96.0;
            w.mon_rect = primary_monitor_rect();
            let (cw, ch) = w.canvas_dip();
            w.d2d.resize(
                (cw * w.dpi_scale).round() as u32,
                (ch * w.dpi_scale).round() as u32,
            );
            w.sync_geometry_and_paint();
            LRESULT(0)
        }
        WM_PAINT => {
            w.sync_geometry_and_paint();
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            w.cleanup();
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[allow(dead_code)]
fn _unused_enum_refs() {
    let _ = VisibilityMode::Fixed;
    let _ = HMENU::default();
}
