# Release v0.2.0 — session, tray, notifications, presence

Native shell for Discord's web app. One Rust binary + a WebView2 window instead
of Electron's main process, GPU process, two utilities and a crash handler.

## Downloads

`huskcord-windows-x64.zip` → unzip → run `huskcord.exe`.
Requires the WebView2 runtime (present on Windows 10/11 by default).

## What is new

| | |
|---|---|
| **Session** | Cookies, localStorage and service worker survive restarts. `--profile work` gives a second logged-in session, `--portable` keeps data next to the exe |
| **Tray** | System tray icon with Show / Mute / Quit. Runs on its own message-pump thread so it cannot stall the renderer |
| **Notifications** | An injected script replaces `window.Notification` before page load, so Discord's toasts become real Windows toasts |
| **Rich Presence** | Speaks Discord's local IPC protocol over `\\.\pipe\discord-ipc-N` |
| **Branding** | Wolf icon embedded in the exe (RT_GROUP_ICON + RT_ICON), drawn in the window and the tray |
| **First run** | Installs to `%LOCALAPPDATA%\Huskcord`, drops a Start menu entry and a desktop shortcut once per profile |

## Measured

Windows 11, Discord 1.0.9267 (Electron 42.7.1), WebView2 157.0.4341.46.

| | Working Set | Private Commit |
|---|---|---|
| Discord desktop | 1,064.9 MB | 756.5 MB |
| Huskcord | ~600 MB | ~400 MB |
| delta | **−455 MB (−42%)** | **−355 MB (−47%)** |

## Known limitations

- **The notification shim is only as good as Discord's usage.** Discord's web
  build uses `window.Notification` for message alerts, but some alert paths go
  through Web Push instead, which the shell does not intercept yet.
- **Rich Presence needs `HUSKCORD_CLIENT_ID`.** There is no bundled Discord
  application id; you supply your own and register its assets.
- **The unread badge is reported but not drawn.** `ITaskbarList3::SetOverlayIcon`
  is the missing half; the shim already forwards the count.
- **Tray can be late.** `Shell_NotifyIconW` returns FALSE while the notification
  area is still starting; the icon retries for 3 s and then gives up quietly.
- **`--profile` does not migrate.** Switching profiles means logging in again.

## Flags

```
huskcord                      %APPDATA%\Huskcord
huskcord --profile work       separate session
huskcord --portable           ./data next to the exe
huskcord --data-dir <path>    explicit
huskcord --no-tray            no tray icon
huskcord --no-presence        no Rich Presence
huskcord --no-shortcuts       skip first-run Start menu / desktop shortcuts
```
