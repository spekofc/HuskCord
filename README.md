<p align="center">
  <img src="assets/logo-256.png" width="160" alt="Huskcord">
</p>

<h1 align="center">Huskcord</h1>

<p align="center">
  A native shell for Discord's web app. Replaces Electron's process stack with a
  single ~800 KB Rust binary and a WebView2 window.
</p>

<p align="center">
  <a href="https://github.com/spekofc/HuskCord/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/spekofc/HuskCord?style=flat-square"></a>
  <a href="LICENSE"><img alt="license" src="https://img.shields.io/github/license/spekofc/HuskCord?style=flat-square"></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-windows%20x64-informational?style=flat-square">
</p>

---

Discord's desktop client is an Electron app: a bundled Chromium, a bundled Node
main process, a GPU process, two utility processes and a crash handler. Huskcord
keeps the part that matters — the Chromium renderer running Discord's real UI —
and replaces everything around it with one native window.

Nothing about Discord's UI is reimplemented. The app is fetched from
`discord.com` exactly as it would be in a browser.

## Measured

Windows 11, Discord 1.0.9267 (Electron 42.7.1), WebView2 157.0.4341.46. Same
machine, same login, both settled after ~30 s.

| | Working Set | Private Commit |
|---|---|---|
| Discord desktop | 1,064.9 MB | 756.5 MB |
| **Huskcord** | **~600 MB** | **~400 MB** |
| delta | **−455 MB (−42%)** | **−355 MB (−47%)** |

Per-process at settle:

```
                 Discord        Huskcord
main / shell      176 MB          21 MB
renderer          509 MB         247 MB
gpu-process       190 MB         110 MB
browser              —           125 MB
network utility    65 MB          50 MB
audio utility      95 MB          22 MB
storage utility      —            20 MB
crashpad           35 MB          18 MB
```

> Numbers move ±100 MB with page load and with Discord's own update cadence.
> `bench/prowl.ps1` produces them; run it yourself rather than trusting a table.

## Install

Grab `huskcord-windows-x64.zip` from [Releases](https://github.com/spekofc/HuskCord/releases),
unzip, run `huskcord.exe`.

Requires the **WebView2 runtime** — present on Windows 10/11 by default.

## Build

```powershell
git clone https://github.com/spekofc/HuskCord
cd HuskCord
cargo build --release
.\target\release\huskcord.exe
```

## Usage

```
huskcord                      %APPDATA%\Huskcord
huskcord --profile work       a separate logged-in session
huskcord --portable           ./data next to the exe
huskcord --data-dir <path>    explicit data directory
huskcord --no-tray            no system tray icon
huskcord --no-presence        no Rich Presence
```

`DISCORD_SHELL_URL` overrides the loaded URL. `HUSKCORD_CLIENT_ID` sets the
Discord application id used for Rich Presence.

## Features

- **Persistent session** — cookies, localStorage and the service worker cache
  live in a WebView2 profile that survives restarts. Profiles keep separate
  logins.
- **System tray** — show, toggle mute, quit. The tray owns its own message-only
  window and pump so it cannot stall the renderer.
- **OS notifications** — a script injected before page load replaces
  `window.Notification`, so Discord's toasts become real Windows toasts.
- **Rich Presence** — speaks Discord's local IPC protocol
  (`\\.\pipe\discord-ipc-N`) directly.
- **Branded icon** — embedded in the exe, the window, and the tray.

## Why winit and not tao

The first shell used `tao`. It opened a window and started a WebView2 browser
process, but the renderer never spawned: `webview2-com`'s controller callback
never completed under `tao`'s message loop, and the process sat at 19 MB with a
valid window handle and no error. `winit` 0.30 hands the pump correctly.

Full write-up, including the process tree that exposed it and the Chromium flag
list that made things *worse*, in [docs/tao-vs-winit.md](docs/tao-vs-winit.md).

## What this is not

Not a Discord client rewrite. There is no local Discord UI to rewrite — the app
arrives over the network. Huskcord gives that app a native shell without
Electron's tax. A true native client (custom UI, custom protocol, ~40–80 MB
steady) is a different project that trades every server-delivered feature.

## Contributing

Read [AGENTS.md](AGENTS.md) before touching the event loop, and
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. Security issues go
through [SECURITY.md](SECURITY.md), never a public issue.

## License

MIT — see [LICENSE](LICENSE).
