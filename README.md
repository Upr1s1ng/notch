# Notch — Dynamic Island for Windows 10/11

A **Pure Rust + GPU Direct2D** Dynamic Island / Notch overlay for Windows 10 & 11 inspired by **Nusa**, **NotchX**, and macOS Dynamic Island — built with zero Electron, zero WebView2, and zero background polling when idle.

---

## Highlights

- **Pure Rust + Direct2D + DirectWrite + WIC** (`windows` crate only — no Chromium, no WebView2, no Node.js).
- **Zero-Draw-When-Idle:** Event-driven `GetMessageW` loop with `1ms` multimedia timer resolution (`timeBeginPeriod(1)`) enabled *only* during active spring transitions and released immediately on settle.
- **3 Switchable Styles:**
  - **Top-Edge Notch (Default):** Flush OLED-black bezel attachment (`y = 0`) with inverted-ear top corners.
  - **Floating Pill:** Lifted pill with dual-HWND frosted-glass backdrop (`CreateRoundRectRgn` + `SetWindowRgn`).
  - **Minimal Notch:** Thin bezel bar for thick-bezel laptops; expands on hover or transient alerts.
- **Nusa Mascot & Transient Peek Pills:** Animated eyes during idle; 2.5s auto-collapsing peek banners for track changes, volume/brightness, power/battery, and reminders.
- **Expanded 8-Tab Island:**
  1. **Home (Bento Grid):** Spotlight Google Search bar (`Ctrl+Alt+Space`), `M T W T F S S` week strip + next agenda item, Discord Voice controls, and Quick Control tiles.
  2. **Music + Split Lyrics:** Windows SMTC (`GlobalSystemMediaTransportControlsSessionManager`) controls, clickable source switcher, live scrubber, and click-to-seek `.lrc` synced lyrics via `lrclib.net` (cached in `%LocalAppData%`).
  3. **HUD:** Control Center–style Master Volume, Brightness (`WmiMonitorBrightnessMethods` + DDC/CI external monitor support), and Per-App Audio Mixer (`IAudioSessionManager2`).
  4. **Tasks:** Local JSON task list with filters (`Today`, `Upcoming`, `Done`) and reminder peeks.
  5. **Timer:** Radial Pomodoro progress ring, round dots, `Until HH:MM` clock, and `25m / 15m / 5m` presets.
  6. **Cal:** Unified agenda combining scheduled tasks + optional `.ics` feeds.
  7. **Apps:** Pin `.exe` apps, folders, or URLs with native icon extraction (`SHGetFileInfoW`).
  8. **Shelf & Clipboard:** Drag-and-drop File Shelf (`IDropTarget` + `CF_HDROP` path references only) + privacy-safe Clipboard History (skips password-manager clipboard formats).
- **Privacy & Gaming Hygiene:**
  - `WDA_EXCLUDEFROMCAPTURE` toggle hides the notch from OBS, Discord, Zoom, and Teams.
  - Fullscreen auto-hide (`SetWinEventHook` + `SHQueryUserNotificationState`) pauses timers/visualizers during fullscreen games and videos.
  - `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` prevents focus stealing (temporarily yields focus only when typing in Search / Add Task / Add App).

---

## Download & Installation

Download the latest build from **[GitHub Releases](https://github.com/Upr1s1ng/notch/releases)** (or the latest **[GitHub Actions](https://github.com/Upr1s1ng/notch/actions)** workflow artifacts):

- `notch-windows-x64.zip` / `notch.exe` — Portable standalone executable
- `Notch_x64.msix` — MSIX package (same packaging format used by **Nusa** for Microsoft Store distribution)
- `SHA256SUMS.txt` — SHA-256 checksums + GitHub Build Provenance Attestations

### Windows Smart App Control & SmartScreen Note
Like **Nusa** (`citoxstd.Nusa` on the Microsoft Store), Windows 11 **Smart App Control (SAC)** and **Defender SmartScreen** require binaries to be signed by a **Microsoft-trusted Code Signing CA** or distributed via the **Microsoft Store**:
- **Unsigned GitHub builds:**
  - If **Smart App Control is Off** (default on most Windows PCs), Windows SmartScreen may show *"Windows protected your PC"* — click **More info → Run anyway**, or verify the binary first using `gh attestation verify notch.exe --owner Upr1s1ng`.
  - If **Smart App Control is On**, Windows blocks *all* unsigned `.exe` files at the kernel level until the release is signed (via `WINDOWS_CERT_PFX` / Azure Trusted Signing) or installed from the Microsoft Store (`Notch_x64.msix`).
- See [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) for the complete Microsoft Store & Authenticode signing setup.

---

## Controls & Shortcuts

| Action | Trigger |
| --- | --- |
| Expand / Collapse | Hover over the top-center pill, or click it |
| Pin Open | Click the **Pin** icon in the top-right header |
| Google Search | `Ctrl + Alt + Space` (or click the Search bar in Home) |
| Add Files to Shelf | Drag & drop any file onto the collapsed pill |
| Settings & Toggles | Right-click the **Notch** icon in the Windows System Tray |

---

## Building from Source

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

> **Note:** If Smart App Control is enabled locally, `cargo` proc-macro / `build.rs` scripts may be blocked (`os error 4551`). Push to GitHub or trigger the **Release Windows Notch** workflow to build cleanly on `windows-latest`.

---

## License

MIT License — see [LICENSE](LICENSE).
