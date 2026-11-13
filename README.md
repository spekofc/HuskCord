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
keeps the part that matters â€” the Chromium renderer running Discord's real UI â€”
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
| delta | **âˆ’455 MB (âˆ’42%)** | **âˆ’355 MB (âˆ’47%)** |

Per-process at settle:

```
                 Discord        Huskcord
main / shell      176 MB          21 MB
renderer          509 MB         247 MB
gpu-process       190 MB         110 MB
browser              â€”           125 MB
network utility    65 MB          50 MB
audio utility      95 MB          22 MB
storage utility      â€”            20 MB
crashpad           35 MB          18 MB
```
<img width="655" height="247" alt="image" src="https://github.com/user-attachments/assets/485cadac-29fd-45bf-8429-2abb8ca85c70" />


> Numbers move Â±100 MB with page load and with Discord's own update cadence.
> `bench/prowl.ps1` produces them; run it yourself rather than trusting a table.

## Install

Grab `huskcord-windows-x64.zip` from [Releases](https://github.com/spekofc/HuskCord/releases),
unzip, run `huskcord.exe`.

Requires the **WebView2 runtime** â€” present on Windows 10/11 by default.

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

- **Persistent session** â€” cookies, localStorage and the service worker cache
  live in a WebView2 profile that survives restarts. Profiles keep separate
  logins.
- **System tray** â€” show, toggle mute, quit. The tray owns its own message-only
  window and pump so it cannot stall the renderer.
- **OS notifications** â€” a script injected before page load replaces
  `window.Notification`, so Discord's toasts become real Windows toasts.
- **Rich Presence** â€” speaks Discord's local IPC protocol
  (`\\.\pipe\discord-ipc-N`) directly.
- **Branded icon** â€” embedded in the exe, the window, and the tray.

## Why winit and not tao

The first shell used `tao`. It opened a window and started a WebView2 browser
process, but the renderer never spawned: `webview2-com`'s controller callback
never completed under `tao`'s message loop, and the process sat at 19 MB with a
valid window handle and no error. `winit` 0.30 hands the pump correctly.

Full write-up, including the process tree that exposed it and the Chromium flag
list that made things *worse*, in [docs/tao-vs-winit.md](docs/tao-vs-winit.md).

## What this is not

Not a Discord client rewrite. There is no local Discord UI to rewrite â€” the app
arrives over the network. Huskcord gives that app a native shell without
Electron's tax. A true native client (custom UI, custom protocol, ~40â€“80 MB
steady) is a different project that trades every server-delivered feature.

## Contributing

Read [AGENTS.md](AGENTS.md) before touching the event loop, and
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. Security issues go
through [SECURITY.md](SECURITY.md), never a public issue.

## License

MIT â€” see [LICENSE](LICENSE).


---

## Two products in this repo

main is Huskcord: Discord's own UI inside a WebView2 window. Full feature
parity, 601 MB, eight processes.

[project-b](https://github.com/spekofc/HuskCord/tree/project-b) is **Wolfcord**:
a native client that draws its own UI with egui. 97 MB, one process, no Chromium.

| | RAM | Processes | Parity |
|---|---|---|---|
| Discord desktop | 1,064.9 MB | 8 | — |
| Huskcord (WebView2) | 600.9 MB | 8 | full |
| [Wolfcord](https://github.com/spekofc/HuskCord/releases/tag/wolfcord-v0.1.0) | **97.0 MB** | **1** | text only so far |

Same measurements, same machine. Neither is a rewrite of the other: the shell runs
Discord's app, the native client replaces it.

Releases are namespaced: * builds the shell, wolfcord-v* builds the client.
