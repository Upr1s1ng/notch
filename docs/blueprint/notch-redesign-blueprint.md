# Clean-Room Redesign Blueprint: Notch (Windows 10/11 Dynamic Island)

## 1. Reverse-Engineering Synthesis (Reference Products)

### Nusa (`citoxstd.Nusa` v1.2.1.0)
- **Distribution `[OBSERVED]`:** Installed as a Microsoft Store MSIX package (`citoxstd.Nusa_1.2.1.0_x64__r9cmnhct3ptgm`), which satisfies Windows Smart App Control without requiring third-party EV certs for local users.
- **Core Loop `[OBSERVED]`:** Top-edge collapsed pill (Clock + status) -> idle transition to **Nusa Mascot** animated eyes (awake, blink, doze `zZ`, wave on return) -> hover-to-expand modules / 2.5s transient Peek banners -> retract on leave or fullscreen occlusion.
- **Signature Mechanics `[OBSERVED]`:** Split-Notch synced `.lrc` lyrics, Minimal Notch mode (thin bezel bar), File Shelf (`%LocalAppData%\Nusa\Shelf` reference-only drop pad), Pomodoro/focus timers, and transient alerts (track, volume, brightness, charger/battery, Caps Lock/layout).

### NotchX
- **Craft Secret `[OBSERVED]`:** **Dual-HWND Frosted Glass** — places a second `WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` backdrop HWND behind the pill, clipped each animation frame via `CreateRoundRectRgn` + `SetWindowRgn`, avoiding DWM's invisible-rectangle blur artifact.
- **Privacy & Grid `[OBSERVED]`:** `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` toggle + 12-column Home widget grid.

### NabyScape (`BillyNabil/nabyscape`) & `hibatillah/notch`
- **Architecture `[OBSERVED]`:** Local-first JSON persistence, Pure Rust + Direct2D rendering (`hibatillah/notch`), zero-draw-when-idle `GetMessageW` event loop, and audio waveform fallback when synced lyrics are unavailable.

---

## 2. DNA Matrix

| Category | Decisions |
|---|---|
| **PRESERVED** | Top-edge inverted-ear notch + Floating Pill + Minimal Notch modes; Dual-HWND clipped glass; `WS_EX_NOACTIVATE` focus discipline; Nusa Mascot eyes; 2.5s transient Peek alerts; SMTC + LRCLIB click-to-seek lyrics; reference-only File Shelf; `WDA_EXCLUDEFROMCAPTURE`. |
| **MODIFIED** | Pure Rust + Direct2D + DirectWrite + WinHttp with **zero `serde`/`winres`/`embed-manifest`** `build.rs` dependencies (custom `json_mini.rs` + `.cargo/config.toml` `/MANIFEST:EMBED`). |
| **DROPPED** | Electron/WebView2/Tauri, global `WH_KEYBOARD_LL`/`WH_MOUSE_LL` hooks, UPX packers, continuous 60fps idle render loops, password-manager clipboard capture. |
| **NEW** | Discord Local IPC (`\\.\pipe\discord-ipc-0`) + CoreAudio/SendInput fallback controls, smart-focus Google Search Bar (`Ctrl+Alt+Space`), Custom App Shortcuts launcher, and GitHub Actions + MSIX Store CI pipeline. |

---

## 3. Implemented File Map (Phases 1–5)
- **Core & Config:** `src/main.rs`, `src/config.rs`, `src/state.rs`, `src/json_mini.rs`, `app.manifest`, `.cargo/config.toml`
- **OS & Direct2D:** `src/os/d2d.rs`, `src/os/window.rs`
- **UI:** `src/ui/theme.rs`, `src/ui/shell.rs`
- **Modules:** `src/modules/{media,lyrics,audio,brightness,discord,search,system_events,tasks,timer,calendar,shortcuts,shelf,clipboard}.rs`
- **CI & MSIX Distribution:** `.github/workflows/release.yml`, `msix/AppxManifest.xml`, `msix/pack-msix.ps1`
