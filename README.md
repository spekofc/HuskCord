<p align="center">
  <img src="assets/logo-256.png" width="150" alt="Huskcord">
</p>

<h1 align="center">Huskcord</h1>

<p align="center">
  Discord without the Electron tax. Two builds of the same idea:<br>
  a shell that runs Discord's own UI in a native window, and a client that draws
  its own UI with no browser at all.
</p>

<p align="center">
  <a href="https://github.com/spekofc/HuskCord/releases/latest"><img alt="shell release" src="https://img.shields.io/github/v/release/spekofc/HuskCord?style=flat-square&label=huskcord"></a>
  <a href="https://github.com/spekofc/HuskCord/releases/tag/wolfcord-v0.1.0"><img alt="wolfcord release" src="https://img.shields.io/badge/wolfcord-0.1.0-38bdf8?style=flat-square"></a>
  <a href="LICENSE"><img alt="license" src="https://img.shields.io/github/license/spekofc/HuskCord?style=flat-square"></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-windows%20x64-informational?style=flat-square">
</p>

---

## The two builds

| | **Huskcord** | **Wolfcord** |
|---|---|---|
| What it is | Discord's real UI inside a native window | Its own UI, speaking the protocol |
| Engine | WebView2 (the Chromium already on Windows) | none |
| **RAM** | **601 MB** | **97 MB** |
| Processes | 8 | **1** |
| Binary | 0.9 MB | 11.9 MB |
| Parity | everything Discord ships | text chat so far |
| Discord updates its UI | you get it free | you reimplement it |
| Download | [huskcord-windows-x64.zip](https://github.com/spekofc/HuskCord/releases/latest) | [wolfcord-windows-x64.zip](https://github.com/spekofc/HuskCord/releases/tag/wolfcord-v0.1.0) |

```text
Discord desktop          1,064.9 MB    8 processes
Huskcord                   600.9 MB    8 processes    full parity
Wolfcord                    97.0 MB    1 process      no browser, own UI
```

Both measured on the same machine, same session, settled. They are not versions
of each other: one runs Discord's app, the other replaces it.

**Pick Huskcord** if you want Discord exactly as Discord made it, just lighter.
**Pick Wolfcord** if you want it small and are willing to live with a missing
voice, screenshare, and embeds for now.

---

# Huskcord — the shell

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
> `bench/measure.ps1` produces them; everything tried to push this lower,
> including the experiments that made it worse, is in
> [docs/memory-experiments.md](docs/memory-experiments.md).

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
huskcord --flag <switch>      append an extra Chromium switch (repeatable)
huskcord --no-tray            no system tray icon
huskcord --no-presence        no Rich Presence
huskcord --no-shortcuts       skip first-run Start menu / desktop shortcuts
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
- **First run** — installs to `%LOCALAPPDATA%\Huskcord`, drops a Start menu entry
  and a desktop shortcut once per profile.

## Why winit and not tao

The first shell used `tao`. It opened a window and started a WebView2 browser
process, but the renderer never spawned: `webview2-com`'s controller callback
never completed under `tao`'s message loop, and the process sat at 19 MB with a
valid window handle and no error. `winit` 0.30 hands the pump correctly.

Full write-up, including the process tree that exposed it and the Chromium flag
list that made things *worse*, in [docs/tao-vs-winit.md](docs/tao-vs-winit.md).

---

# Wolfcord — the native client

No Chromium. No WebView2. One process, **97 MB**, drawing its own UI with egui.

The shell above plateaus at ~600 MB because Discord's UI is a React app and needs
a browser engine to run. Every attempt to shrink the engine failed or backfired
([docs/memory-experiments.md](docs/memory-experiments.md)). Wolfcord does not run
a browser engine at all.

## Measured

| | RAM | Processes |
|---|---|---|
| Discord desktop | 1,064.9 MB | 8 |
| Huskcord shell | 600.9 MB | 8 |
| **Wolfcord** | **97.0 MB** | **1** |

`97 MB` with no token. Loading a gateway connection, a guild list and message
history moves it to roughly 100-140 MB depending on how much history is cached.
History is capped at 200 messages per channel on purpose.

## Install

Grab `wolfcord-windows-x64.zip` from the
[wolfcord-v0.1.0 release](https://github.com/spekofc/HuskCord/releases/tag/wolfcord-v0.1.0),
unzip, set a token, run it.

```powershell
$env:WOLFCORD_TOKEN = "your-bot-token"
.\wolfcord.exe
```

Without a token the app opens an offline view that says exactly that, rather than
a blank window.

## What works

- Gateway connection with automatic resume
- Guild list, channel list, category ordering
- Message history, live messages, edits, deletes
- Unread counts
- Send messages, delete your own
- Text intents only: no member or presence churn

## What does not work yet

| | Status |
|---|---|
| Voice | not started. Needs Opus + WebRTC. This is the hard one |
| Screenshare | not started |
| Attachments / embeds | text only |
| DMs | channels exist, no dedicated inbox |
| Markdown | raw as Discord sent it |
| Mentions | counted, not highlighted |
| Login | token via env var, no password/2FA UI |

## Build

```powershell
git checkout project-b
cargo build --release -p wolfcord-app
```

> The `ring` crate fails with `cl.exe D8050` unless the MSVC environment comes
> from `vcvars64.bat` **and** debug info is off (`CARGO_PROFILE_RELEASE_DEBUG=0`).
> Both, not either. The CI workflow does exactly this.

## Layout

```
crates/
  wolfcord-core/   protocol: gateway, REST, event model, state. No UI, no window.
  wolfcord-app/    egui frontend. Depends on core, never the reverse.
```

Split so the protocol can be tested without a window and the UI developed against
fake events.

> **Read this before using it.** Automating a *user account* violates Discord's
> Terms of Service and accounts get terminated. **Bot tokens are the supported
> path.** Pointing Wolfcord at a user token is your call and your risk.

---

## Repository layout

```
main        Huskcord   the shell       v*           releases
project-b   Wolfcord   the client      wolfcord-v*  releases
```

Releases are namespaced so the two products never collide. Both workflows build,
smoke-test, package and publish on their own tag pattern.

## Docs

| | |
|---|---|
| [docs/tao-vs-winit.md](docs/tao-vs-winit.md) | why the first shell opened a window and never rendered |
| [docs/memory-experiments.md](docs/memory-experiments.md) | every memory reduction tried, including the ones that made it worse |
| [docs/task-manager-grouping.md](docs/task-manager-grouping.md) | what Windows keys on for app grouping, measured |
| [docs/project-b.md](docs/project-b.md) | the native client's original plan and constraints |

## Contributing

Read [AGENTS.md](AGENTS.md) before touching the event loop, and
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR. Security issues go
through [SECURITY.md](SECURITY.md), never a public issue.

## License

MIT — see [LICENSE](LICENSE).
